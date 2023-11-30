use csv::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, error::Error};

use crate::utils::csv_validator::{
    validate_csv_header, validate_csv_row, AddressColumnValidator, AmountColumnValidator, ColumnValidator,
    ValidationError,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CampaignCsvRecord {
    pub address: String,
    pub amount: u128,
}

pub struct CampaignCsvParsed {
    pub records: Vec<CampaignCsvRecord>,
    pub validation_errors: Vec<ValidationError>,
    pub number_of_recipients: i32,
    pub total_amount: u128,
}

// TO DO: Check why this requires Send + Sync to work. Error happening in the create handler.

impl CampaignCsvParsed {
    pub fn build(rdr: Reader<&[u8]>, decimals: usize) -> Result<CampaignCsvParsed, Box<dyn Error + Send + Sync>> {
        let mut rdr = rdr;
        let mut validation_errors = Vec::new();
        let mut records: Vec<CampaignCsvRecord> = Vec::new();
        let mut total_amount: u128 = 0;
        let mut number_of_recipients: i32 = 0;
        let pattern = format!(r"^[+]?\d*\.?\d{{0,{}}}$", decimals);
        let amount_regex = Regex::new(&pattern).unwrap();

        let amount_validator = AmountColumnValidator { regex: amount_regex };
        let address_validator = AddressColumnValidator;

        let validators: Vec<&dyn ColumnValidator> = vec![&address_validator, &amount_validator];
        let mut unique_addresses: HashSet<String> = HashSet::new();

        // Validate the CSV header
        let header = rdr.headers()?;
        let header_errors = validate_csv_header(header, &validators);
        if let Some(error) = header_errors {
            validation_errors.push(error);
            return Ok(CampaignCsvParsed { total_amount, number_of_recipients, records, validation_errors });
        }

        for (row_index, result) in rdr.records().enumerate() {
            let row = row_index + 2;
            if result.is_err() {
                validation_errors.push(ValidationError { row, message: String::from("Invalid row") });
                continue;
            }
            let record = result.unwrap();

            if validation_errors.len() >= 100 {
                break;
            }

            let address_field = record[0].trim();
            let amount_field = record[1].trim();
            let row_errors = validate_csv_row(&record, row_index, &validators);
            if !row_errors.is_empty() {
                validation_errors.extend(row_errors);
            }

            if unique_addresses.contains(&address_field.to_lowercase()) {
                validation_errors.push(ValidationError {
                    row,
                    message: String::from(
                        "Each recipient should have an unique address. This address was already specified in file",
                    ),
                });
            }

            if validation_errors.is_empty() {
                let address = address_field.to_string().to_lowercase();
                let padded_amount = pad_value(amount_field, decimals);
                total_amount += padded_amount;
                number_of_recipients += 1;
                unique_addresses.insert(address.clone());
                records.push(CampaignCsvRecord { address, amount: padded_amount });
            }
        }

        Ok(CampaignCsvParsed { total_amount, number_of_recipients, records, validation_errors })
    }
}

fn pad_value(s: &str, no_decimals: usize) -> u128 {
    let decimal_point = s.find('.').unwrap_or(s.len());
    if decimal_point == s.len() {
        return format!("{}{}", s, "0".repeat(no_decimals)).parse().unwrap();
    }

    let decimals = s.len() - decimal_point - 1;
    let mut result = String::with_capacity(s.len() + no_decimals - decimals);
    result.push_str(&s[0..decimal_point]);
    result.push_str(&s[decimal_point + 1..]);

    for _ in 0..(no_decimals - decimals) {
        result.push('0');
    }

    result.parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv::ReaderBuilder;

    fn create_reader(input: &str) -> Reader<&[u8]> {
        ReaderBuilder::new().from_reader(input.as_bytes())
    }

    #[test]
    fn test_pad_value() {
        assert_eq!(pad_value("480.5", 3), 480500);
        assert_eq!(pad_value("613", 2), 61300);
        assert_eq!(pad_value("123.", 1), 1230);
    }

    #[test]
    fn test_valid_csv() {
        let csv_data = "address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(result.records.len(), 2);
        assert_eq!(result.total_amount, 30000);
        assert_eq!(result.number_of_recipients, 2);
        assert!(result.validation_errors.is_empty());
    }

    #[test]
    fn test_csv_wrong_header() {
        let csv_data = "address,amount_invalid\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(
            result.validation_errors[0].message,
            "CSV header invalid. The csv header should contain `amount` column. The amount column id missing"
        );
    }

    #[test]
    fn test_csv_missing_header() {
        let csv_data =
            "address\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "Insufficient columns");
    }

    #[test]
    fn test_csv_row_missing_column() {
        let csv_data = "address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "Invalid row");
    }

    #[test]
    fn test_csv_row_invalid_address() {
        let csv_data = "address,amount\n0xThisIsNotAnAddress,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "Invalid Ethereum address");
        assert_eq!(result.validation_errors[0].row, 2);
    }

    #[test]
    fn test_csv_duplicated_addresses() {
        let csv_data = "address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491, 200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(
            result.validation_errors[0].message,
            "Each recipient should have an unique address. This address was already specified in file"
        );
        assert_eq!(result.validation_errors[0].row, 3);
    }

    #[test]
    fn test_csv_row_alphanumeric_amount() {
        let csv_data = "address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491, alphanumeric_amount\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "Amounts should be positive, in normal notation, with an optional decimal point and a maximum number of decimals as provided by the query parameter.");
        assert_eq!(result.validation_errors[0].row, 2);
    }

    #[test]
    fn test_csv_row_amount_0() {
        let csv_data = "address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491, 0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "The amount cannot be 0");
        assert_eq!(result.validation_errors[0].row, 2);
    }

    #[test]
    fn test_csv_row_amount_negative() {
        let csv_data = "address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491, -1\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "Amounts should be positive, in normal notation, with an optional decimal point and a maximum number of decimals as provided by the query parameter.");
        assert_eq!(result.validation_errors[0].row, 2);
    }

    #[test]
    fn test_csv_row_amount_wrong_precision() {
        let csv_data = "address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491, 1.1234\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let reader = create_reader(csv_data);
        let result = CampaignCsvParsed::build(reader, 2);
        assert!(result.is_ok());
        let result = result.unwrap();

        assert!(!result.validation_errors.is_empty());
        assert_eq!(result.validation_errors[0].message, "Amounts should be positive, in normal notation, with an optional decimal point and a maximum number of decimals as provided by the query parameter.");
        assert_eq!(result.validation_errors[0].row, 2);
    }
}
