# ParseTgLink

`ParseTgLink` - это высокопроизводительный, **Zero-Copy access** парсер ссылок Telegram для замены `Regex` на языке Rust.
- *Был создан по принципам [TelegramUserLinkParser](https://github.com/Puxxalwl/TelegramUserLinkParser)*

## Особенности:
- **No std**: Полная поддержка no_std.
- **Zero-copy access**: Никаких аллокаций в куче (Heap). Все данные — это слайсы (&str) из исходного текста.
- **Unsafe**: Использование сырых указателей (*const u8) и ручное управление итерацией для обхода проверок границ (bounds checks).
- **SIMD-подобные сравнения**: Чтение и сравнение 4 или 2 байтов за один раз через read_unaligned с применением битовых масок для регистронезависимости.

## Поддерживаемые форматы (LinkKind)

| Формат | Результат |
| :--- | :--- |
| @username | Username("username") |
| @123456 | LinkKind::Id(123456) |
| t.me/username | LinkKind::Username("username") |
| t.me/@id12345 | LinkKind::Id(12345) |
| tg://resolve?domain=juzo_otvetit | LinkKind::Username("juzo_otvetit") |
| tg://user?id=12345 | LinkKind::Id(12345) |
| tg://openmessage?user_id=12345 | LinkKind::Id(12345) |

## Пример использования

```rust
use crate::...::{ParseTgLink, LinkKind};

fn main() {
    // регистронезависимость
    let text = "Contact @JuzoCode or visit T.Me/JuZo_OtVeTiT. Hello (Гуся)[http://t.me/shuseks]";

    // Поиск всех ссылок (итератор)
    for link in ParseTgLink::all(text) {
        match link {
            LinkKind::Username(username) => println!("Username found (all): {username}"),
            LinkKind::Id(id) => println!("ID found (all): {id}"),
            _ => {}
        }
    }

    // Быстрое получение только первой ссылки
    let first = ParseTgLink::new(text);
}
```

