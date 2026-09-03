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
| @username | LinkKind::Username("username") |
| @12345 | LinkKind::Id(12345) |
| @_12345 | LinkKind::ChatId(-12345) |
| @-12345 | LinkKind::ChatId(-12345) |
<!-- | :--- | :--- | -->
| t.me/username | LinkKind::Username("username") |
| t.me/12345 | LinkKind::Id(12345) |
| t.me/_12345 | LinkKind::ChatId(-12345) |
| t.me/-12345 | LinkKind::ChatId(-12345) |
| t.me/@id12345 | LinkKind::Id(12345) |
<!-- | :--- | :--- | -->
| telegram.me/username | LinkKind::Username("username") |
| telegram.me/12345 | LinkKind::Id(12345) |
| telegram.me/_12345 | LinkKind::ChatId(12345) |
| telegram.me/-12345 | LinkKind::ChatId(12345) |
| telegram.me/@id12345 | LinkKind::Id(12345) |
<!-- | :--- | :--- | -->
| telegram.dog/username | LinkKind::Username("username") |
| telegram.dog/12345 | LinkKind::Id(12345) |
| telegram.dog/_12345 | LinkKind::ChatId(-12345) |
| telegram.dog/-12345 | LinkKind::ChatId(-12345) |
| telegram.dog/@id12345 | LinkKind::Id(12345) |
<!-- | :--- | :--- | -->
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
            LinkKind::ChatId(id) => println!("ChatId found (all): {id}"),
            LinkKind::Id(id) => println!("Id found (all): {id}"),
        }
    }

    // Быстрое получение только первой ссылки
    let first = ParseTgLink::new(text);
}
```