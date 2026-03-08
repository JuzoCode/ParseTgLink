use core::marker::PhantomData;
use core::slice::from_raw_parts;
use core::str::from_utf8_unchecked;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LinkKind<'a> { 
    Username(&'a str), 
    Id(u64) 
}

/// RU:  Структура для высокопроизводительного парсинга ссылок Telegram.
/// ENG: A structure for high-performance parsing of Telegram links.
pub struct ParseTgLink<'a> {
    ptr: *const u8,
    end: *const u8,

    // RU:  Параметр для работы с Lifetime-bound references. (Zero-Sized)
    // ENG: Parameter for working with Lifetime-bound references. (Zero-Sized)
    __marker: PhantomData<&'a [u8]>,
}

impl<'a> ParseTgLink<'a> {
    // RU:  Быстрый метод для получения первой найденной ссылки. (one link)
    // ENG: Quick method to retrieve the first found link. (one link)
    #[inline(always)]
    pub fn new(text: &'a str) -> Option<LinkKind<'a>> {
        Self::all(text).next()
    }

    // RU:  Быстрый метод для получения всех найденных ссылок. (all link)
    // ENG: A quick method for getting all found links. (all link)
    #[inline(always)]
    pub fn all(text: &'a str) -> Self {
        let b = text.as_bytes();
        let s = b.as_ptr();
        Self {
            ptr: s,
            end: unsafe { s.add(b.len()) },
            __marker: PhantomData,
        }
    }


    #[inline(always)]
    unsafe fn num(&self, mut s: *const u8) -> Option<(u64, *const u8)> {
        if s >= self.end || !(*s).is_ascii_digit() { return None }
        
        let mut v = (*s - b'0') as u64;
        s = s.add(1);

        while s < self.end {
            let b = *s;
            if !b.is_ascii_digit() { break }
            v = v.wrapping_mul(10).wrapping_add((b - b'0') as u64);
            s = s.add(1);
        }

        Some((v, s))
    }

    #[inline(always)]
    unsafe fn str(&self, s: *const u8) -> Option<(&'a str, *const u8)> {
        if s >= self.end || !(*s).is_ascii_alphabetic() { return None }

        let mut c = s.add(1);
        while c < self.end {
            let b = *c;
            if !b.is_ascii_alphanumeric() && b != b'_' { break }
            c = c.add(1);
        }

        let len = c as usize - s as usize;
        Some((from_utf8_unchecked(from_raw_parts(s, len)), c))
    }


    // "resolve?domain=" 
    #[inline(always)]
    unsafe fn is_resolve_domain(&self, s: *const u8) -> bool {
        // "reso"
        if ((s as *const u32).read_unaligned() | 0x20202020) != 0x6F736572 { return false }
        // "lve?"
        if ((s.add(4) as *const u32).read_unaligned() | 0x20202020) != 0x3F65766C { return false }
        // "doma"
        if ((s.add(8) as *const u32).read_unaligned() | 0x20202020) != 0x616D6F64 { return false }
        // "in="
        if ((s.add(12) as *const u16).read_unaligned() | 0x2020) != 0x6E69 { return false }
        *s.add(14) == b'='
    }

    // "user?id="
    #[inline(always)]
    unsafe fn is_tg_user(&self, s: *const u8) -> bool {
        let v1 = (s as *const u32).read_unaligned() | 0x20202020; // "user"
        let v2 = (s.add(4) as *const u32).read_unaligned();       // "?id="

        v1 == 0x72657375 && (v2 | 0x00202000) == 0x3D64693F
    }

    // "openmessage?user_id="
    #[inline(always)]
    unsafe fn is_open_message(&self, s: *const u8) -> bool {
        if ((s as *const u32).read_unaligned() | 0x20202020) != 0x6E65706F { return false }         // "open"
        if ((s.add(4) as *const u32).read_unaligned() | 0x20202020) != 0x7373656D { return false }  // "mess"
        if ((s.add(8) as *const u32).read_unaligned() | 0x00202020) != 0x3F656761 { return false }  // "age?"
        if ((s.add(12) as *const u32).read_unaligned() | 0x20202020) != 0x72657375 { return false } // "user"
        if ((s.add(16) as *const u32).read_unaligned() | 0x20202020) != 0x3D64697F { return false } // "_id="
        true
    }


    #[inline(always)]
    unsafe fn t_me(&mut self, s: *const u8) -> Option<LinkKind<'a>> {
        // RU:  Проверка "t.me/"
        // ENG: Verification "t.me/"
        if ((s as *const u32).read_unaligned() | 0x20200020) != 0x656D2E74 && *s.add(5) != b'/' { return None }

        let sub_ptr = s.add(5);
        let sub_len = self.end as usize - sub_ptr as usize;
        if sub_len == 0 { return None }

        match *sub_ptr | 0x20 { 
            // "t.me/@id{num}"
            0x60 => {
                let id_ptr = sub_ptr.add(1);
                // "id"
                if sub_len >= 3 && ((id_ptr as *const u16).read_unaligned() | 0x2020) == 0x6469 {
                    let (v, n) = self.num(id_ptr.add(2))?;
                    self.ptr = n;
                    return Some(LinkKind::Id(v))
                }
                None
            }
            // "t.me/{username}"
            b'a'..=b'z' => {
                let (u, n) = self.str(sub_ptr)?;
                self.ptr = n;
                return Some(LinkKind::Username(u))
            }
            _ => None
        }
    }

    #[inline(always)]
    unsafe fn tg_protocol(&mut self, s: *const u8) -> Option<LinkKind<'a>> {
        // RU:  Проверка "tg://"
        // ENG: Verification "tg://"
        if ((s as *const u32).read_unaligned() | 0x00002020) != 0x2F3A6774 && *s.add(5) != b'/' { return None }
        
        let sub_ptr = s.add(5);
        let sub_len = self.end as usize - sub_ptr as usize;
        if sub_len == 0 { return None }

        match *sub_ptr | 0x20 {
            b'u' if sub_len >= 8 && self.is_tg_user(sub_ptr) => {
                let (v, n) = self.num(sub_ptr.add(8))?;
                self.ptr = n;
                Some(LinkKind::Id(v))
            }
            b'r' if sub_len >= 15 && self.is_resolve_domain(sub_ptr) => {
                let (u, n) = self.str(sub_ptr.add(15))?;
                self.ptr = n;
                Some(LinkKind::Username(u))
            }
            b'o' if sub_len >= 20 && self.is_open_message(sub_ptr) => {
                let (v, n) = self.num(sub_ptr.add(20))?;
                self.ptr = n;
                Some(LinkKind::Id(v))
            }
            _ => None
        }
    }
}

/// Вспомогательная функция для быстрого поиска символов @, t, T
#[inline(always)]
fn find_start_byte(slice: &[u8]) -> Option<usize> {
    for (i, &b) in slice.iter().enumerate() {
        if b == b'@' || (b | 0x20) == b't' { return Some(i) }
    }
    None
}

impl<'a> Iterator for ParseTgLink<'a> {
    type Item = LinkKind<'a>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        while self.ptr < self.end {
            let len = (self.end as usize).wrapping_sub(self.ptr as usize);
            let search_slice = unsafe { core::slice::from_raw_parts(self.ptr, len) };

            if let Some(pos) = find_start_byte(search_slice) {
                unsafe {
                    let s = self.ptr.add(pos);
                    let b = *s;

                    match b {
                        // "@"
                        0x40 => {
                            let next_s = s.add(1);
                            if let Some((val, next_ptr)) = self.num(next_s) {
                                self.ptr = next_ptr;
                                return Some(LinkKind::Id(val))
                            }
                            if let Some((val, next_ptr)) = self.str(next_s) {
                                self.ptr = next_ptr;
                                return Some(LinkKind::Username(val))
                            }
                        }
                        // 0x74 ('t') | 0x20 = 0x74, 0x54 ('T') | 0x20 = 0x74
                        0x74 | 0x54 => {
                            if let Some(link) = self.t_me(s) { return Some(link) }
                            if let Some(link) = self.tg_protocol(s) { return Some(link) }
                        }
                        _ => { }
                    }

                    self.ptr = s.add(1);
                }
            } else { break }
        }
        self.ptr = self.end;
        None
    }
}
