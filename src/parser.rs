use core::{
    iter::FusedIterator, marker::PhantomData, slice::from_raw_parts, str::from_utf8_unchecked,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LinkKind<'a> {
    Id(u64),
    ChatId(i64),
    Username(&'a str),
}
#[inline(always)]
fn is_letter(b: u8) -> bool {
    (b | 0x20).wrapping_sub(b'a') < 26
}

type Hit<'a> = Option<(LinkKind<'a>, *const u8)>;

#[derive(Clone)]
pub struct ParseTgLink<'a> {
    ptr: *const u8,
    end: *const u8,

    // RU:  Начало текста. Нужно для обхода назад (`username.t.me`).
    // ENG: Start of the text. Needed for the backward walk (`username.t.me`).
    start: *const u8,
    floor: *const u8,

    // RU:  Параметр для работы с Lifetime-bound references. (Zero-Sized)
    // ENG: Parameter for working with Lifetime-bound references. (Zero-Sized)
    _marker: PhantomData<&'a [u8]>,
}

unsafe impl Send for ParseTgLink<'_> {}
unsafe impl Sync for ParseTgLink<'_> {}

impl<'a> ParseTgLink<'a> {
    // RU:  Быстрый метод для получения первой найденной ссылки. (one link)
    // ENG: Quick method to retrieve the first found link. (one link)
    #[inline]
    pub fn new(text: &'a str) -> Option<LinkKind<'a>> {
        Self::all(text).next()
    }

    // RU:  Быстрый метод для получения всех найденных ссылок. (all link)
    // ENG: A quick method for getting all found links. (all link)
    #[inline]
    pub const fn all(text: &'a str) -> Self {
        let b = text.as_bytes();
        let s = b.as_ptr();

        Self {
            ptr: s,
            end: unsafe { s.add(b.len()) },
            start: s,
            floor: s,
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    fn rem(&self, p: *const u8) -> usize {
        self.end as usize - p as usize
    }

    #[inline(always)]
    unsafe fn seek(&self, mut p: *const u8) -> *const u8 {
        while self.rem(p) >= 8 {
            let w = rd64(p);
            let t = (w | 0x2020202020202020) ^ 0x7474747474747474; // 't' | 'T'
            let a = w ^ 0x4040404040404040; // '@'
            let m = (t.wrapping_sub(0x0101010101010101) & !t & 0x8080808080808080)
                | (a.wrapping_sub(0x0101010101010101) & !a & 0x8080808080808080);
            if m != 0 {
                return p.add((m.trailing_zeros() >> 3) as usize);
            }
            p = p.add(8);
        }

        while p < self.end {
            let c = *p;
            if c == b'@' || (c | 0x20) == b't' {
                return p;
            }
            p = p.add(1);
        }

        self.end
    }

    #[inline(always)]
    unsafe fn digits(&self, mut s: *const u8) -> Option<(u64, *const u8)> {
        let from = s;
        let mut v: u64 = 0;

        while s < self.end {
            let d = (*s).wrapping_sub(b'0');
            if d > 9 {
                break;
            }

            v = v
                .wrapping_mul(10)
                .wrapping_add(d as u64);
            s = s.add(1);
        }

        let n = s as usize - from as usize;
        if n == 0 || n > 19 { None } else { Some((v, s)) }
    }

    #[inline(always)]
    unsafe fn num(&self, s: *const u8) -> Hit<'a> {
        let (v, s) = self.digits(s)?;
        Some((LinkKind::Id(v), s))
    }

    #[inline(always)]
    unsafe fn chat(&self, s: *const u8) -> Hit<'a> {
        let (v, s) = self.digits(s)?;
        if v > i64::MAX as u64 {
            return None;
        }
        Some((LinkKind::ChatId(-(v as i64)), s))
    }

    #[inline(always)]
    unsafe fn name(&self, s: *const u8) -> Hit<'a> {
        if s >= self.end || !is_letter(*s) {
            return None;
        }

        let mut c = s.add(1);
        while c < self.end && CLASS[*c as usize] & 2 != 0 {
            c = c.add(1);
        }

        let len = c as usize - s as usize;

        Some((
            LinkKind::Username(from_utf8_unchecked(from_raw_parts(s, len))),
            c,
        ))
    }

    // "{id}" | "-{id}" | "_{id}" | "{username}"
    #[inline(always)]
    unsafe fn target(&self, p: *const u8) -> Hit<'a> {
        if p >= self.end {
            return None;
        }

        match *p | 0x20 {
            b'a'..=b'z' => self.name(p),
            b'0'..=b'9' => self.num(p),
            0x7F | 0x2D => self.chat(p.add(1)),
            _ => None,
        }
    }

    // RU:  То, что идёт после "t.me/" / "telegram.me/" / "telegram.dog/".
    // ENG: What follows "t.me/" / "telegram.me/" / "telegram.dog/".
    #[inline(always)]
    unsafe fn path(&self, p: *const u8) -> Hit<'a> {
        if p < self.end && *p == b'@' {
            // "@id{num}"
            let q = p.add(1);
            if self.rem(q) >= 2 && (rd16(q) | 0x2020) == 0x6469 {
                return self.num(q.add(2));
            }
            return None;
        }

        self.target(p)
    }

    // "user?id="
    #[inline(always)]
    const unsafe fn is_tg_user(&self, s: *const u8) -> bool {
        (rd32(s) | 0x20202020) == 0x72657375 // "user"
            && (rd32(s.add(4)) | 0x00202000) == 0x3D64693F // "?id="
    }

    // "resolve?domain="
    #[inline(always)]
    const unsafe fn is_resolve_domain(&self, s: *const u8) -> bool {
        (rd32(s) | 0x20202020) == 0x6F736572 // "reso"
            && (rd32(s.add(4)) | 0x00202020) == 0x3F65766C // "lve?"
            && (rd32(s.add(8)) | 0x20202020) == 0x616D6F64 // "doma"
            && (rd16(s.add(12)) | 0x2020) == 0x6E69 // "in"
            && *s.add(14) == b'='
    }

    // "openmessage?user_id="
    #[inline(always)]
    const unsafe fn is_open_message(&self, s: *const u8) -> bool {
        (rd32(s) | 0x20202020) == 0x6E65706F // "open"
            && (rd32(s.add(4)) | 0x20202020) == 0x7373656D // "mess"
            && (rd32(s.add(8)) | 0x00202020) == 0x3F656761 // "age?"
            && (rd32(s.add(12)) | 0x20202020) == 0x72657375 // "user"
            && (rd32(s.add(16)) | 0x00202000) == 0x3D64695F // "_id="
    }

    #[inline]
    unsafe fn tg(&self, s: *const u8) -> Hit<'a> {
        let rem = self.rem(s);
        if rem < 6 || (rd32(s) | 0x00002020) != 0x2F3A6774 || *s.add(4) != b'/' {
            return None;
        }

        let p = s.add(5);
        let n = rem - 5;

        match *p | 0x20 {
            // "tg://user?id={num}"
            0x75 if n >= 8 && self.is_tg_user(p) => self.num(p.add(8)),
            // "tg://resolve?domain={username}"
            0x72 if n >= 15 && self.is_resolve_domain(p) => self.name(p.add(15)),
            // "tg://openmessage?user_id={num}"
            0x6F if n >= 20 && self.is_open_message(p) => self.num(p.add(20)),
            _ => None,
        }
    }

    #[inline]
    unsafe fn subdomain(&self, s: *const u8) -> Hit<'a> {
        let dot = s.sub(1);

        let mut b = dot;
        while b > self.start && CLASS[*b.sub(1) as usize] & 2 != 0 {
            b = b.sub(1);
        }

        let len = dot as usize - b as usize;

        // RU:  `b < floor` — username пересёкся бы с предыдущей ссылкой.
        // ENG: `b < floor` — the username would overlap the previous link.
        if len == 0 || !is_letter(*b) || b < self.floor {
            return None;
        }

        if b > self.start {
            let p = *b.sub(1);
            if p == b'.' || p == b'-' {
                return None;
            }
        }

        let after = s.add(4);
        if after < self.end {
            let c = *after;

            // "hello.t.mex", "hello.t.me-x"
            if CLASS[c as usize] != 0 {
                return None;
            }

            if c == b'.' {
                let n = after.add(1);
                if n < self.end && CLASS[*n as usize] & 2 != 0 {
                    return None;
                }
            }
        }

        Some((
            LinkKind::Username(from_utf8_unchecked(from_raw_parts(b, len))),
            after,
        ))
    }

    #[inline]
    unsafe fn t_me(&self, s: *const u8) -> Hit<'a> {
        // "t.me"
        if self.rem(s) < 4 || (rd32(s) | 0x20200020) != 0x656D2E74 {
            return None;
        }

        if s > self.start {
            let prev = *s.sub(1);

            if prev == b'.' {
                // "{username}.t.me"
                return self.subdomain(s);
            } else if CLASS[prev as usize] != 0 {
                // RU:  Часть чужого домена ("cat.me/foo") — не наша ссылка.
                // ENG: Part of another domain ("cat.me/foo") — not our link.
                return None;
            }
        }
        // "t.me/{...}"
        let slash = s.add(4);
        if slash < self.end && *slash == b'/' {
            self.path(slash.add(1))
        } else {
            None
        }
    }

    #[inline]
    unsafe fn telegram(&self, s: *const u8) -> Hit<'a> {
        let rem = self.rem(s);
        if rem < 13 {
            return None;
        }

        // "telegram"
        if (rd64(s) | 0x2020202020202020) != 0x6D617267656C6574 {
            return None;
        }

        // "xtelegram.me/..." — не наша ссылка
        if s > self.start && CLASS[*s.sub(1) as usize] != 0 {
            return None;
        }

        let tail = rd32(s.add(8));

        let p = if (tail | 0x00202000) == 0x2F656D2E {
            // ".me/"
            s.add(12)
        } else if (tail | 0x20202000) == 0x676F642E && rem >= 14 && *s.add(12) == b'/' {
            // ".dog/"
            s.add(13)
        } else {
            return None;
        };

        self.path(p)
    }
}

impl<'a> Iterator for ParseTgLink<'a> {
    type Item = LinkKind<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                let s = self.seek(self.ptr);
                if s >= self.end {
                    self.ptr = self.end;
                    return None;
                }

                let hit = if *s == b'@' {
                    self.target(s.add(1))
                } else {
                    let n = s.add(1);
                    if n < self.end {
                        match *n {
                            b'.' => self.t_me(s),
                            b'g' | b'G' => self.tg(s),
                            b'e' | b'E' => self.telegram(s),
                            _ => None,
                        }
                    } else {
                        None
                    }
                };

                match hit {
                    Some((kind, next)) => {
                        self.ptr = next;
                        self.floor = next;
                        return Some(kind);
                    }
                    None => self.ptr = s.add(1),
                }
            }
        }
    }
}

impl FusedIterator for ParseTgLink<'_> {}

static CLASS: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        if b.is_ascii_alphanumeric() || b == b'_' {
            t[i] = 3;
        } else if b == b'-' {
            t[i] = 1;
        }
        i += 1;
    }
    t
};

#[inline(always)]
const unsafe fn rd16(p: *const u8) -> u16 {
    u16::from_le(
        p.cast::<u16>()
            .read_unaligned(),
    )
}
#[inline(always)]
const unsafe fn rd32(p: *const u8) -> u32 {
    u32::from_le(
        p.cast::<u32>()
            .read_unaligned(),
    )
}
#[inline(always)]
const unsafe fn rd64(p: *const u8) -> u64 {
    u64::from_le(
        p.cast::<u64>()
            .read_unaligned(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        LinkKind::{ChatId, Id, Username},
        *,
    };

    fn one(s: &str) -> Option<LinkKind<'_>> {
        ParseTgLink::new(s)
    }

    #[test]
    fn subdomain_basic() {
        assert_eq!(one("hello.t.me"), Some(Username("hello")));
        assert_eq!(one("juzo.t.me"), Some(Username("juzo")));
        assert_eq!(one("juzocode.t.me"), Some(Username("juzocode")));
        assert_eq!(one("meow://juzocode.t.me/123"), Some(Username("juzocode")));
        assert_eq!(one("Hello.T.ME!"), Some(Username("Hello")));
        assert_eq!(one("see hello.t.me."), Some(Username("hello")));
        assert_eq!(one("user_1.t.me"), Some(Username("user_1")));
    }
    
    #[test]
    fn subdomain_invalid() {
        assert_eq!(one("123456.t.me"), None);
        assert_eq!(one("_abc.t.me"), None);
        assert_eq!(one("9abc.t.me"), None);
        assert_eq!(one("foo-bar.t.me"), None);
        assert_eq!(one("a.b.t.me"), None);
        assert_eq!(one(".t.me"), None);
        assert_eq!(one("hello.t.mex"), None);
        assert_eq!(one("hello.t.me.evil.com"), None);
        assert_eq!(one("cat.me/foo"), None);
    }

    #[test]
    fn subdomain_no_duplicates() {
        let mut it = ParseTgLink::all("@juzo_cm.t.me");
        assert_eq!(it.next(), Some(Username("juzo_cm")));
        assert_eq!(it.next(), None);

        let mut it = ParseTgLink::all("juzo_cm_bot.t.me and t.me/JuzoCode");
        assert_eq!(it.next(), Some(Username("juzo_cm_bot")));
        assert_eq!(it.next(), Some(Username("JuzoCode")));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn t_me() {
        assert_eq!(one("t.me/juzocode"), Some(Username("juzocode")));
        assert_eq!(one("https:/t.me/JuzoCode"), Some(Username("JuzoCode")));
        assert_eq!(one("https://t.me/JuzoCode"), Some(Username("JuzoCode")));
        assert_eq!(one("http:/t.me/JuzoCode"), Some(Username("JuzoCode")));
        assert_eq!(one("http://t.me/JuzoCode"), Some(Username("JuzoCode")));
        assert_eq!(one("t.me/392851555"), Some(Id(392851555)));
        assert_eq!(one("t.me/-100123"), Some(ChatId(-100123)));
        assert_eq!(one("t.me/@id42"), Some(Id(42)));
        assert_eq!(one("t.mexJuzoCode"), None);
        assert_eq!(one("t.me/"), None);
        assert_eq!(one("t.me"), None);
        assert_eq!(one("t.me/99999999999999999999999"), None);
    }

    #[test]
    fn telegram_domains() {
        assert_eq!(one("telegram.me/juzocodE"), Some(Username("juzocodE")));
        assert_eq!(
            one("https:/teleGram.dog/JuzoCode"),
            Some(Username("JuzoCode"))
        );
        assert_eq!(
            one("https://teleGram.dog/JuzoCode"),
            Some(Username("JuzoCode"))
        );
        assert_eq!(
            one("http:/TeLeGrAm.DoG/Juzocode"),
            Some(Username("Juzocode"))
        );
        assert_eq!(
            one("http://teleGram.dog/JuzoCode"),
            Some(Username("JuzoCode"))
        );
        assert_eq!(one("TELEGRAM.dogXJuzoCode"), None);
        assert_eq!(one("xtelegram.me/JuzoCode"), None);
    }

    #[test]
    fn tg_protocol() {
        assert_eq!(one("tg://user?id=42"), Some(Id(42)));
        assert_eq!(
            one("tg://resolve?domain=JuZo_OtVeTiT"),
            Some(Username("JuZo_OtVeTiT"))
        );
        assert_eq!(
            one("tg://resolve?domain=juzo_otvetit"),
            Some(Username("juzo_otvetit"))
        );
        assert_eq!(one("tg://openmessage?user_id=7"), Some(Id(7)));
        assert_eq!(one("tg://oPEnmeSSaGe?user_id=7"), Some(Id(7)));
        assert_eq!(one("tg://resolve?domain="), None);
        assert_eq!(one("tg:/Xuser?id=1"), None);
    }

    #[test]
    fn at_mentions() {
        assert_eq!(one("@JuzoCode"), Some(Username("JuzoCode")));
        assert_eq!(one("@392851555"), Some(Id(392851555)));
        assert_eq!(one("@-100"), Some(ChatId(-100)));
        assert_eq!(one("abc @"), None);
    }

    #[test]
    fn swar_alignment() {
        for pad in 0..24 {
            let mut buf = [b' '; 48];
            buf[pad..pad + 10].copy_from_slice(b"hello.t.me");
            let s = core::str::from_utf8(&buf).unwrap();
            assert_eq!(one(s), Some(Username("hello")), "pad={}", pad);
        }
    }
}
