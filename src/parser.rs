use core::{marker::PhantomData, slice::from_raw_parts, str::from_utf8_unchecked};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LinkKind<'a> {
    Id(u64),
    ChatId(i64),
    Username(&'a str),
}

pub struct ParseTgLink<'a> {
    ptr: *const u8,
    end: *const u8,

    // RU:  Параметр для работы с Lifetime-bound references. (Zero-Sized)
    // ENG: Parameter for working with Lifetime-bound references. (Zero-Sized)
    _marker: PhantomData<&'a [u8]>,
}

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
            _marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn num(&self, mut s: *const u8) -> Option<(u64, *const u8)> {
        if s >= self.end || !(*s).is_ascii_digit() {
            return None;
        }

        let mut v = (*s - b'0') as u64;
        s = s.add(1);

        while s < self.end {
            let b = *s;
            if !b.is_ascii_digit() {
                break;
            }
            v = v
                .wrapping_mul(10)
                .wrapping_add((b - b'0') as u64);
            s = s.add(1);
        }

        Some((v, s))
    }

    #[inline]
    unsafe fn chat_id(&self, mut s: *const u8) -> Option<(i64, *const u8)> {
        if s >= self.end || !(*s).is_ascii_digit() {
            return None;
        }

        let mut v = (*s - b'0') as i64;
        s = s.add(1);

        while s < self.end {
            let b = *s;
            if !b.is_ascii_digit() {
                break;
            }

            let digit = (b - b'0') as i64;

            if v > i64::MAX / 10 || v == i64::MAX / 10 && digit > 7 {
                return None;
            }

            v = v * 10 + digit;
            s = s.add(1);
        }

        Some((-v, s))
    }

    #[inline]
    unsafe fn str(&self, s: *const u8) -> Option<(&'a str, *const u8)> {
        let first = *s | 0x20;

        if first.wrapping_sub(b'a') > 25 {
            return None;
        }

        let mut c = s.add(1);

        while c < self.end {
            let b = *c;

            if (b | 0x20).wrapping_sub(b'a') <= 25 || b.wrapping_sub(b'0') <= 9 || b == b'_' {
                c = c.add(1);
            } else {
                break;
            }
        }

        let len = c.offset_from(s) as usize;

        Some((from_utf8_unchecked(from_raw_parts(s, len)), c))
    }

    #[inline]
    const unsafe fn is_resolve_domain(&self, s: *const u8) -> bool {
        // "reso"
        if ((s as *const u32).read_unaligned() | 0x20202020) != 0x6F736572 {
            return false;
        }
        // "lve?"
        if ((s.add(4) as *const u32).read_unaligned() | 0x20202020) != 0x3F65766C {
            return false;
        }
        // "doma"
        if ((s.add(8) as *const u32).read_unaligned() | 0x20202020) != 0x616D6F64 {
            return false;
        }
        // "in="
        if ((s.add(12) as *const u16).read_unaligned() | 0x2020) != 0x6E69 {
            return false;
        }
        *s.add(14) == b'='
    }

    // "user?id="
    #[inline]
    const unsafe fn is_tg_user(&self, s: *const u8) -> bool {
        let v1 = (s as *const u32).read_unaligned() | 0x20202020; // "user"
        let v2 = (s.add(4) as *const u32).read_unaligned(); // "?id="

        v1 == 0x72657375 && (v2 | 0x00202000) == 0x3D64693F
    }

    // "openmessage?user_id="
    #[inline]
    const unsafe fn is_open_message(&self, s: *const u8) -> bool {
        // "open"
        if ((s as *const u32).read_unaligned() | 0x20202020) != 0x6E65706F {
            return false;
        }
        // "mess"
        if ((s.add(4) as *const u32).read_unaligned() | 0x20202020) != 0x7373656D {
            return false;
        }
        // "age?"
        if ((s.add(8) as *const u32).read_unaligned() | 0x00202020) != 0x3F656761 {
            return false;
        }
        // "user"
        if ((s.add(12) as *const u32).read_unaligned() | 0x20202020) != 0x72657375 {
            return false;
        }
        // "_id="
        if ((s.add(16) as *const u32).read_unaligned() | 0x20202020) != 0x3D64697F {
            return false;
        }
        true
    }

    #[inline]
    unsafe fn t_me(&mut self, s: *const u8) -> Option<LinkKind<'a>> {
        // RU:  Проверка "t.me/"
        // ENG: Verification "t.me/"
        if (self.end as usize - s as usize) < 5 {
            return None;
        }
        if ((s as *const u32).read_unaligned() | 0x20200020) != 0x656D2E74 && *s.add(5) != b'/' {
            return None;
        }

        let sub_ptr = s.add(5);
        let sub_len = self.end as usize - sub_ptr as usize;
        if sub_len == 0 {
            return None;
        }

        match *sub_ptr | 0x20 {
            // "t.me/@id{num}"
            0x60 => {
                let id_ptr = sub_ptr.add(1);
                if sub_len >= 3 && ((id_ptr as *const u16).read_unaligned() | 0x2020) == 0x6469 {
                    let (v, n) = self.num(id_ptr.add(2))?;
                    self.ptr = n;
                    return Some(LinkKind::Id(v));
                }
                None
            }
            // "t.me/{id}"
            b'0'..=b'9' => {
                let (u, n) = self.num(sub_ptr)?;
                self.ptr = n;
                Some(LinkKind::Id(u))
            }
            // "t.me/{username}"
            b'a'..=b'z' => {
                let (u, n) = self.str(sub_ptr)?;
                self.ptr = n;
                Some(LinkKind::Username(u))
            }
            _ => None,
        }
    }

    #[inline]
    unsafe fn telegram_domain(&mut self, s: *const u8) -> Option<LinkKind<'a>> {
        let remain = self.end.offset_from(s) as usize;

        if remain < 13 {
            return None;
        }

        // "telegram"
        if ((s as *const u64).read_unaligned() | 0x2020202020202020) != 0x6D617267656C6574 {
            return None;
        }

        // ".me/" / ".dog/"
        let tail = (s.add(8) as *const u32).read_unaligned() | 0x20202020;

        let offset = match tail {
            // ".me/"
            0x2F656D2E => 12,
            // ".dog/"
            0x676F642E => {
                if remain < 14 {
                    return None;
                }
                13
            }

            _ => return None,
        };

        let sub_ptr = s.add(offset);
        let sub_len = self.end as usize - sub_ptr as usize;
        if sub_len == 0 {
            return None;
        }

        match *sub_ptr | 0x20 {
            // "telegram.dog/@id{num}" / "telegram.me/@id{num}"
            0x60 => {
                let id_ptr = sub_ptr.add(1);
                if sub_len >= 3 && ((id_ptr as *const u16).read_unaligned() | 0x2020) == 0x6469 {
                    let (v, n) = self.num(id_ptr.add(2))?;
                    self.ptr = n;
                    return Some(LinkKind::Id(v));
                }
                None
            }
            // ".dog/{id}" / ".me/{id}"
            b'0'..=b'9' => {
                let (v, n) = self.num(sub_ptr)?;
                self.ptr = n;
                Some(LinkKind::Id(v))
            }
            // ".dog/{username}" / ".me/{usermame}"
            b'a'..=b'z' => {
                let (u, n) = self.str(sub_ptr)?;
                self.ptr = n;
                Some(LinkKind::Username(u))
            }
            _ => None,
        }
    }

    #[inline]
    unsafe fn tg_protocol(&mut self, s: *const u8) -> Option<LinkKind<'a>> {
        // RU:  Проверка "tg://"
        // ENG: Verification "tg://"
        if (self.end as usize - s as usize) < 5 {
            return None;
        }
        if ((s as *const u32).read_unaligned() | 0x00002020) != 0x2F3A6774 && *s.add(5) != b'/' {
            return None;
        }

        let sub_ptr = s.add(5);
        let sub_len = self.end as usize - sub_ptr as usize;
        if sub_len == 0 {
            return None;
        }

        match *sub_ptr | 0x20 {
            // "tg://user?id={num}"
            b'u' if sub_len >= 8 && self.is_tg_user(sub_ptr) => {
                let (v, n) = self.num(sub_ptr.add(8))?;
                self.ptr = n;
                Some(LinkKind::Id(v))
            }
            // "tg://resolve?domain={username}"
            b'r' if sub_len >= 15 && self.is_resolve_domain(sub_ptr) => {
                let (u, n) = self.str(sub_ptr.add(15))?;
                self.ptr = n;
                Some(LinkKind::Username(u))
            }
            // "tg://openmessage?user_id={num}"
            b'o' if sub_len >= 20 && self.is_open_message(sub_ptr) => {
                let (v, n) = self.num(sub_ptr.add(20))?;
                self.ptr = n;
                Some(LinkKind::Id(v))
            }
            _ => None,
        }
    }
}

impl<'a> Iterator for ParseTgLink<'a> {
    type Item = LinkKind<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            while self.ptr < self.end {
                let b = *self.ptr;

                match b {
                    0x40 => {
                        let s = self.ptr;
                        let next_s = s.add(1);

                        if next_s < self.end && *next_s == b'_' {
                            if let Some((val, next_ptr)) = self.chat_id(next_s.add(1)) {
                                self.ptr = next_ptr;
                                return Some(LinkKind::ChatId(val));
                            }
                            self.ptr = s.add(1);
                            continue;
                        }

                        if let Some((val, next_ptr)) = self.num(next_s) {
                            self.ptr = next_ptr;
                            return Some(LinkKind::Id(val));
                        }
                        if let Some((val, next_ptr)) = self.str(next_s) {
                            self.ptr = next_ptr;
                            return Some(LinkKind::Username(val));
                        }
                        self.ptr = s.add(1);
                    }
                    0x74 | 0x54 => {
                        let s = self.ptr;
                        if let Some(link) = self.t_me(s) {
                            return Some(link);
                        }
                        if let Some(link) = self.tg_protocol(s) {
                            return Some(link);
                        }
                        if let Some(link) = self.telegram_domain(s) {
                            return Some(link);
                        }
                        self.ptr = s.add(1);
                    }
                    _ => {
                        self.ptr = self.ptr.add(1);
                    }
                }
            }

            self.ptr = self.end;
            None
        }
    }
}
