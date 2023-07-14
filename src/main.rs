use chrono::{DateTime, Utc};
use clap::Parser;
use ethereum_types::U256;
use std::error::Error;
use std::fmt::Display;
use std::str::FromStr;

/// Represents supported UniSat chain transaction categories.
#[derive(Debug, enum_display_derive::Display, PartialEq)]
enum Action {
    Mint,
    Transfer,
}

impl FromStr for Action {
    type Err = String;

    fn from_str(input: &str) -> Result<Action, Self::Err> {
        match input {
            "mint" => Ok(Action::Mint),
            "transfer" => Ok(Action::Transfer),
            _ => Err(format!("Unknown action: {}", &input)),
        }
    }
}

/// Represents supported CTC transaction categories.
#[derive(Debug)]
enum Category {
    Buy,
    Mint,
}

impl Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Category::Buy => "buy",
                Category::Mint => "mint",
            }
        )
    }
}

/// Transaction status.
///
/// We use this to ensure the transactions we encounter have succeeded.
/// If we ever encounter a failed transaction, we'll have to update
/// our logic to handle those.
#[derive(Debug, PartialEq)]
enum State {
    Success,
}

impl FromStr for State {
    type Err = String;

    fn from_str(input: &str) -> Result<State, Self::Err> {
        match input {
            "success" => Ok(State::Success),
            _ => Err(format!("Unknown state: {}", &input)),
        }
    }
}

/// We only expect to encounter BRC20 tokens, but if we encounter
/// others, we'll be alerted by this types failure to parse.
#[derive(Debug, enum_display_derive::Display, PartialEq)]
enum TokenType {
    BRC20,
}

impl FromStr for TokenType {
    type Err = String;

    fn from_str(input: &str) -> Result<TokenType, Self::Err> {
        match input {
            "BRC20" => Ok(TokenType::BRC20),
            _ => Err(format!("Unknown token type: {}", &input)),
        }
    }
}

/// Command line arguments.
#[derive(Debug, Default, clap::Parser)]
struct Args {
    api_key: String,
    wallet: String,
}

/// Represents a CSV row, formatted to CTC's schema.
#[derive(Debug, PartialEq)]
struct CsvRow {
    timestamp: String,
    category: String,
    base_currency: String,
    base_amount: String,
    from: String,
    to: String,
    hash: String,
    description: String,
}

/// Represents the relevant data for an inscription transfer.
#[derive(Debug)]
struct Inscription {
    action: Action,
    amount: U256,
    date_time: DateTime<Utc>,
    from_address: String,
    inscription_id: String,
    // This value is being used implicitly during deserialization to ensure the txn was successful
    #[allow(dead_code)]
    state: State,
    to_address: String,
    token: String,
    token_type: TokenType,
    tx_id: String,
}

/// OKLink API inscription. We use serde to decode JSON into this primitive struct
/// before converting it to the `Inscription` struct.
#[allow(non_snake_case)]
#[derive(Clone, Debug, serde::Deserialize)]
struct InscriptionRaw {
    actionType: String,
    amount: String,
    fromAddress: String,
    inscriptionId: String,
    state: String,
    time: String,
    toAddress: String,
    token: String,
    tokenType: String,
    txId: String,
}

/// The processed pagination data from an OKLink request.
#[derive(Debug)]
struct Pagination {
    inscriptions: Vec<Inscription>,
    page: i32,
    total_pages: i32,
}

/// OKLink API pagination. We use serde to decode JSON into this primitive struct
/// before converting it to the `Pagination` struct.
#[allow(non_snake_case)]
#[derive(Clone, Debug, serde::Deserialize)]
struct PaginationRaw {
    inscriptionsList: Vec<InscriptionRaw>,
    limit: String,
    page: String,
    totalPage: String,
    totalTransaction: String,
}

/// OKLink API response. We use serde to decode JSON into this primitive struct
/// before grabbing the data out and converting that to a `Pagination` struct.
#[derive(Clone, Debug, serde::Deserialize)]
struct ResponseRaw {
    data: Vec<PaginationRaw>,
}

/// Our program's entrypoint.
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    println!(
        "Fetching inscriptions for wallet {} using API Key {}",
        args.wallet, args.api_key
    );
    let client = reqwest::blocking::Client::new();
    let mut inscriptions: Vec<Inscription> = vec![];
    fetch_pages(args, client, &mut inscriptions, 0)?;
    println!("{:#?}", inscriptions);
    println!("Total inscritions: {}", inscriptions.len());
    write_csv(inscriptions)?;
    Ok(())
}

/// Fetch all the inscriptions for a given wallet address.
///
/// Will fetch one page at a time, recursively calling itself until all
/// pages have been fetched.
fn fetch_pages(
    args: Args,
    client: reqwest::blocking::Client,
    inscriptions: &mut Vec<Inscription>,
    page: i32,
) -> Result<&mut Vec<Inscription>, Box<dyn Error>> {
    println!("fetching page {}", page);
    let res = client
        .get(format!("https://www.oklink.com/api/v5/explorer/btc/transaction-list?&page={}&limit=50&address={}", page + 1, &args.wallet))
        .header("Ok-Access-Key", &args.api_key)
        .header("Content-Type", "application/json")
        .send()?;
    let body = res.text()?;
    let raw: ResponseRaw = serde_json::from_str(&body)?;
    let pagination = process_response(&raw)?;
    inscriptions.extend(pagination.inscriptions);
    println!(
        "fetched page {} out of {}",
        pagination.page, pagination.total_pages
    );
    if pagination.page == pagination.total_pages {
        Ok(inscriptions)
    } else {
        fetch_pages(args, client, inscriptions, page + 1)
    }
}

/// Convert primitive OK Link data to more useful `Inscription`.
fn process_inscription(raw: &InscriptionRaw) -> Result<Inscription, Box<dyn Error>> {
    let action = Action::from_str(&raw.actionType)?;
    let amount = U256::from_dec_str(&raw.amount).map_err(|e| format!("{:?}", e))?;
    let date_time = unix_to_datetime(&raw.time)?;
    let state = State::from_str(&raw.state)?;
    let token_type = TokenType::from_str(&raw.tokenType)?;
    Ok(Inscription {
        action,
        amount,
        date_time,
        from_address: raw.fromAddress.clone(),
        inscription_id: raw.inscriptionId.clone(),
        state,
        to_address: raw.toAddress.clone(),
        token: raw.token.clone(),
        token_type,
        tx_id: raw.txId.clone(),
    })
}

/// Convert primitive OK Link pagination to more useful data.
fn process_pagination(raw: &PaginationRaw) -> Result<Pagination, Box<dyn Error>> {
    let inscriptions = raw
        .inscriptionsList
        .clone()
        .into_iter()
        .map(|x| process_inscription(&x))
        .collect::<Result<Vec<Inscription>, _>>()?;
    let page = i32::from_str(&raw.page)?;
    let total_pages = i32::from_str(&raw.totalPage)?;
    Ok(Pagination {
        inscriptions,
        page,
        total_pages,
    })
}

/// Convert primitive OK Link response to more useful data.
fn process_response(raw: &ResponseRaw) -> Result<Pagination, Box<dyn Error>> {
    let data = raw
        .data
        .get(0)
        .ok_or("No Pagination found")
        .map(process_pagination)??;
    Ok(data)
}

/// Convert `Inscription` to a struct with the values we want to write to a CSV.
fn to_csv_row(inscription: Inscription) -> CsvRow {
    let category = match inscription.action {
        Action::Mint => Category::Mint,
        Action::Transfer => Category::Buy, // May need more involved logic if we encounter non-buy transfers
    };
    CsvRow {
        timestamp: format!("{}", inscription.date_time.format("%Y/%m/%d %H:%M:%S")),
        category: format!("{}", category),
        base_currency: inscription.token.clone(),
        base_amount: format!("{}", inscription.amount),
        from: inscription.from_address.clone(),
        to: inscription.to_address.clone(),
        hash: inscription.tx_id.clone(),
        description: format!(
            "{} {} with inscription_id {}",
            inscription.token_type, inscription.action, inscription.inscription_id
        ),
    }
}

/// Convert unix timestamp to UTC datetime.
fn unix_to_datetime(timestamp: &String) -> Result<DateTime<Utc>, Box<dyn Error>> {
    let ms = u64::from_str(timestamp)?;
    let duration = std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms);
    Ok(DateTime::<Utc>::from(duration))
}

/// Given a `Vec<Inscription>` write them all to a CSV.
fn write_csv(inscriptions: Vec<Inscription>) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all("csv")?;
    let now = chrono::offset::Local::now();
    let filename = format!("csv/{}.csv", now.format("%Y-%m-%d %H-%M-%S"));
    println!("Writing {}", filename);
    let mut writer = csv::Writer::from_path(&filename)?;
    writer.write_record([
        "Timestamp (UTC)",
        "Type",
        "Base Currency",
        "Base Amount",
        "Quote Currency",
        "Quote Amount",
        "Fee Currency",
        "Fee Amount",
        "From",
        "To",
        "Blockchain",
        "ID",
        "Description",
    ])?;
    for row in inscriptions.into_iter().map(to_csv_row) {
        writer.write_record(&[
            row.timestamp,           // timestamp
            row.category,            // type
            row.base_currency,       // base currency
            row.base_amount,         // base amount
            String::from(""),        // quote currency
            String::from(""),        // quote amount
            String::from(""),        // fee currency
            String::from(""),        // fee amount
            row.from,                // from
            row.to,                  // to
            String::from("Bitcoin"), // blockchain
            row.hash,                // id
            row.description,         // description
        ])?;
    }
    writer.flush()?;
    println!("Successfully wrote {}", filename);
    Ok(())
}

#[test]
fn test_to_csv_row() -> Result<(), Box<dyn Error>> {
    let inscription = Inscription {
        action: Action::Transfer,
        amount: U256::from_dec_str("1000").map_err(|e| format!("{:?}", e))?,
        date_time: DateTime::parse_from_rfc3339("2023-07-07T01:23:45Z")?.with_timezone(&Utc),
        from_address: String::from("from"),
        inscription_id: String::from("inscription_id"),
        state: State::Success,
        to_address: String::from("to"),
        token: String::from("sats"),
        token_type: TokenType::BRC20,
        tx_id: String::from("hash"),
    };
    let expected = CsvRow {
        timestamp: String::from("2023/07/07 01:23:45"),
        category: String::from("buy"),
        base_currency: String::from("sats"),
        base_amount: String::from("1000"),
        from: String::from("from"),
        to: String::from("to"),
        hash: String::from("hash"),
        description: String::from("BRC20 Transfer with inscription_id inscription_id"),
    };
    assert_eq!(to_csv_row(inscription), expected);
    Ok(())
}

#[test]
fn test_unix_to_datetime() -> Result<(), Box<dyn Error>> {
    let timestamp = String::from("1685092041000");
    let actual = unix_to_datetime(&timestamp)?;
    let expected = DateTime::parse_from_rfc3339("2023-05-26T09:07:21Z")?.with_timezone(&Utc);
    assert_eq!(actual, expected);
    Ok(())
}
