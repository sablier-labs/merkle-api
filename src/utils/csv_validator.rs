use csv::StringRecord;
use ethers_rs::Address;
use regex::Regex;
use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ValidationError {
    pub row: usize,
    pub message: String,
}

pub fn is_valid_eth_address(address: &str) -> bool {
    Address::try_from(address).is_ok()
}

pub trait ColumnValidator {
    fn validate_cel(&self, cel: &str, row_index: usize) -> Option<ValidationError>;
    fn validate_header(&self, cel: &str) -> Option<ValidationError>;
}

pub struct AddressColumnValidator;
impl ColumnValidator for AddressColumnValidator {
    fn validate_cel(&self, cel: &str, row_index: usize) -> Option<ValidationError> {
        let is_valid = is_valid_eth_address(cel);
        if !is_valid {
            return Some(ValidationError { row: row_index + 2, message: String::from("Invalid Ethereum address") });
        }
        None
    }

    fn validate_header(&self, cel: &str) -> Option<ValidationError> {
        if cel.to_lowercase() != "address" {
            return Some(ValidationError {
                row: 1, // Header is in the first row
                message: String::from(
                    "CSV header invalid. The csv header should be `address` column. The address column is missing",
                ),
            });
        }
        None
    }
}

pub struct AmountColumnValidator {
    pub regex: Regex,
}

impl ColumnValidator for AmountColumnValidator {
    fn validate_cel(&self, cel: &str, row_index: usize) -> Option<ValidationError> {
        let is_valid = self.regex.is_match(cel);
        if !is_valid {
            return Some(ValidationError {
                row: row_index + 2,
                message: String::from("Amounts should be positive, in normal notation, with an optional decimal point and a maximum number of decimals as provided by the query parameter."),
            });
        }

        let amount: f64 = cel.parse().unwrap();

        if amount == 0.0 {
            return Some(ValidationError { row: row_index + 2, message: String::from("The amount cannot be 0") });
        }
        None
    }

    fn validate_header(&self, cel: &str) -> Option<ValidationError> {
        if cel.to_lowercase() != "amount" {
            return Some(ValidationError {
                row: 1, // Header is in the first row
                message: String::from(
                    "CSV header invalid. The csv header should contain `amount` column. The amount column id missing",
                ),
            });
        }
        None
    }
}

pub fn validate_csv_row(
    row: &StringRecord,
    row_index: usize,
    validators: &[&dyn ColumnValidator],
) -> Vec<ValidationError> {
    let mut errors: Vec<ValidationError> = Vec::new();
    if row.len() < validators.len() {
        errors.push(ValidationError {
            row: row_index + 2, // +2 to account for CSV header
            message: String::from("Insufficient columns"),
        });
        return errors;
    }
    for (index, validator) in validators.iter().enumerate() {
        let cel = row[index].trim();
        let cel_error = validator.validate_cel(cel, row_index);
        if let Some(error) = cel_error {
            errors.push(error);
        }
    }
    errors
}

pub fn validate_csv_header(header: &StringRecord, validators: &[&dyn ColumnValidator]) -> Option<ValidationError> {
    if header.len() < validators.len() {
        let error = ValidationError { row: 1, message: String::from("Insufficient columns") };
        return Some(error);
    }
    for (index, validator) in validators.iter().enumerate() {
        let head = header[index].trim();
        let header_error = validator.validate_header(head);
        if let Some(error) = header_error {
            return Some(error);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_ETH_ADDRESS: &str = "0xf31b00e025584486f7c37Cf0AE0073c97c12c634";
    const INVALID_ETH_ADDRESS: &str = "0xthisIsNotAnAddress";
    const AMOUNT_PATTERN: &str = r"^[+]?\d*\.?\d{0,3}$";

    fn create_validators() -> (AddressColumnValidator, AmountColumnValidator) {
        let address_validator = AddressColumnValidator;
        let amount_regex = Regex::new(AMOUNT_PATTERN).unwrap();
        let amount_validator = AmountColumnValidator { regex: amount_regex };
        (address_validator, amount_validator)
    }

    fn assert_validation_cel<T: ColumnValidator>(validator: &T, value: &str, is_valid: bool) {
        let result = validator.validate_cel(value, 0);
        assert_eq!(result.is_none(), is_valid);
    }

    fn assert_validation_header<T: ColumnValidator>(validator: &T, header: &str, is_valid: bool) {
        let result = validator.validate_header(header);
        assert_eq!(result.is_none(), is_valid);
    }

    #[test]
    fn eth_address_validation() {
        assert!(is_valid_eth_address(VALID_ETH_ADDRESS));
        assert!(!is_valid_eth_address(INVALID_ETH_ADDRESS));
    }

    #[test]
    fn address_column_validator_tests() {
        let (address_validator, _) = create_validators();
        assert_validation_cel(&address_validator, VALID_ETH_ADDRESS, true);
        assert_validation_cel(&address_validator, INVALID_ETH_ADDRESS, false);
        assert_validation_header(&address_validator, "address", true);
        assert_validation_header(&address_validator, "amount", false);
    }

    #[test]
    fn amount_column_validator_tests() {
        let (_, amount_validator) = create_validators();
        assert_validation_cel(&amount_validator, "123.45", true);
        assert_validation_cel(&amount_validator, "thisIsNotANumber", false);
        assert_validation_cel(&amount_validator, "0.0", false);
        assert_validation_header(&amount_validator, "amount", true);
        assert_validation_header(&amount_validator, "address", false);
    }

    #[test]
    fn csv_row_validation() {
        let (address_validator, amount_validator) = create_validators();
        let validators: Vec<&dyn ColumnValidator> = vec![&address_validator, &amount_validator];

        let valid_row = StringRecord::from(vec![VALID_ETH_ADDRESS, "489.312"]);
        assert!(validate_csv_row(&valid_row, 0, &validators).is_empty());

        let insufficient_columns: StringRecord = StringRecord::from(vec![VALID_ETH_ADDRESS]);
        assert!(!validate_csv_row(&insufficient_columns, 0, &validators).is_empty());

        let invalid_address = StringRecord::from(vec!["thisIsNotAnAddress", "12534"]);
        assert!(!validate_csv_row(&invalid_address, 0, &validators).is_empty());

        let invalid_amount = StringRecord::from(vec![VALID_ETH_ADDRESS, "12.576757"]);
        assert!(!validate_csv_row(&invalid_amount, 0, &validators).is_empty());
    }

    #[test]
    fn csv_header_validation() {
        let (address_validator, amount_validator) = create_validators();
        let validators: Vec<&dyn ColumnValidator> = vec![&address_validator, &amount_validator];

        let valid_header = StringRecord::from(vec!["address", "amount"]);
        assert!(validate_csv_header(&valid_header, &validators).is_none());

        let invalid_address_header = StringRecord::from(vec!["address_invalid", "amount"]);
        assert!(validate_csv_header(&invalid_address_header, &validators).is_some());

        let invalid_amount_header = StringRecord::from(vec!["address", "amount_invalid"]);
        assert!(validate_csv_header(&invalid_amount_header, &validators).is_some());
    }
}
