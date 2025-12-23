# YP Bank System

Библиотека и утилиты командной строки для парсинга, сериализации и конвертации финансовых данных между различными форматами.

## Поддерживаемые форматы

- **MT940**: SWIFT-подобные банковские выписки
- **CAMT.053**: ISO 20022 XML формат
- **CSV**: Произвольные банковские/бухгалтерские выгрузки

## Структура проекта

```
ypbank-system/
├── Cargo.toml                  # Конфигурация проекта
├── src/
│   ├── lib.rs                  # Библиотека
│   ├── error.rs                # Типы ошибок
│   ├── types.rs                # Общие типы данных
│   ├── mt940_format.rs         # Парсер/сериализатор MT940
│   ├── camt053_format.rs       # Парсер/сериализатор CAMT.053
│   ├── csv_format.rs           # Парсер/сериализатор CSV
│   ├── conversion.rs           # Конвертация между форматами
│   └── bin/
│       ├── converter.rs        # CLI converter
│       └── comparer.rs         # CLI comparer
├── specs/                      # Спецификация и примеры
│   ├── project_assignment.md
│   └── examples/
└── README.md
```

## Установка

```bash
# Клонировать репозиторий
git clone https://github.com/youruser/ypbank-system.git
cd ypbank-system

# Собрать пр оект
cargo build --release
```

## Использование

### Библиотека

#### Парсинг MT940 файла

```rust
use std::fs::File;
use ypbank_system::mt940_format::Mt940Statement;

let mut file = File::open("statement.mt940")?;
let statement = Mt940Statement::from_read(&mut file)?;

println!("Statement ID: {}", statement.statement.statement_id);
for transaction in &statement.statement.transactions {
    println!("Transaction: {} - {}", transaction.reference, transaction.amount);
}
```

#### Конвертация MT940 в CAMT.053

```rust
use std::fs::File;
use ypbank_system::mt940_format::Mt940Statement;
use ypbank_system::camt053_format::Camt053Statement;

// Чтение MT940
let mut input = File::open("input.mt940")?;
let mt940 = Mt940Statement::from_read(&mut input)?;

// Конвертация через трейт From
let camt053: Camt053Statement = mt940.into();

// Запись CAMT.053
let mut output = File::create("output.xml")?;
camt053.write_to(&mut output)?;
```

### CLI Converter (ypbank_converter)

Утилита для конвертации между форматами банковских выписок.

```bash
# Конвертация из MT940 в CAMT.053
ypbank_converter \
  --input statement.mt940 \
  --input-format mt940 \
  --output-format camt053 \
  --output output.xml

# Конвертация из CAMT.053 в CSV
ypbank_converter \
  --input statement.xml \
  --input-format camt053 \
  --output-format csv \
  --output output.csv

# Использование stdin/stdout
cat statement.mt940 | ypbank_converter \
  --input-format mt940 \
  --output-format camt053 \
  > output.xml
```

### CLI Comparer (ypbank_compare)

Утилита для сравнения банковских выписок из разных форматов.

```bash
# Сравнение двух файлов
ypbank_compare \
  --file1 statement1.mt940 \
  --format1 mt940 \
  --file2 statement2.xml \
  --format2 camt053

# Пример вывода при совпадении
# The transaction records in 'file1' and 'file2' are identical.

# Пример вывода при различиях
# Differences found:
#   - Transaction 1 amount differs: 100.00 vs 100.50
#   - Transaction 3 date differs: 2024-01-15 vs 2024-01-16
```

## Архитектура

### Использование трейтов Read и Write

Библиотека использует стандартные трейты `Read` и `Write` из стандартной библиотеки Rust, что обеспечивает:

- **Гибкость**: работа с файлами, stdin/stdout, буферами памяти, сетевыми потоками
- **Отсутствие дублирования**: одна реализация для всех источников/приемников данных
- **Производительность**: статический полиморфизм (мономорфизация)
- **Тестируемость**: легко тестировать с помощью `Vec<u8>` или других буферов

Пример:

```rust
impl Mt940Statement {
    pub fn from_read<R: std::io::Read>(reader: &mut R) -> Result<Self> {
        // Работает с любым типом, реализующим Read
    }

    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        // Работает с любым типом, реализующим Write
    }
}
```

### Конвертация через трейт From

Конвертация между MT940 и CAMT.053 реализована через стандартный трейт `From`:

```rust
// MT940 -> CAMT.053
impl From<Mt940Statement> for Camt053Statement { ... }

// CAMT.053 -> MT940
impl From<Camt053Statement> for Mt940Statement { ... }
```

При конвертации из MT940 в CAMT.053 недостающая информация заполняется значениями по умолчанию.
При конвертации из CAMT.053 в MT940 дополнительная информация помещается в поле `:86:`.

## Обработка ошибок

Библиотека использует собственный тип `Result<T>` с настраиваемыми ошибками:

```rust
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    Io(#[from] io::Error),
    CsvError(#[from] csv::Error),
    XmlError(String),
    Mt940ParseError { line: usize, message: String },
    InvalidDate(String),
    InvalidAmount(String),
    MissingField(String),
    InvalidFormat(String),
    ParseError(String),
    ConversionError(String),
}
```

Все публичные функции возвращают `Result<T>`, никогда не используют `.unwrap()`.

## Тестирование

```bash
# Запустить все тесты
cargo test

# Запустить тесты с выводом
cargo test -- --nocapture

# Запустить тесты для конкретного модуля
cargo test mt940_format
```

## Документация

```bash
# Генерация и открытие документации
cargo doc --open

# Документация для библиотеки
cargo doc --no-deps --lib
```

## Примеры файлов

Примеры файлов в различных форматах находятся в папке `specs/examples/`:

- MT940: `mt 940 gs.mt940`, `MT_940 oracle.mt940`
- CAMT.053: `camt 053 danske bank`
- CSV: `Пример выписки по счёту 1.csv`

## Требования

- Rust 1.70+
- Зависимости (указаны в Cargo.toml):
  - chrono - работа с датами
  - csv - парсинг CSV
  - quick-xml - парсинг XML
  - serde - сериализация/десериализация
  - rust_decimal - точная работа с денежными суммами
  - clap - парсинг аргументов командной строки
  - thiserror - обработка ошибок

## Лицензия

MIT

## Автор

YP Team
