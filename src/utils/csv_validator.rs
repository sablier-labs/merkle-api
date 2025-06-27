use csv::StringRecord;
use ethers_rs::Address;
use regex::Regex;
use serde::Serialize;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Enum to represent different blockchain address types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AddressType {
    Ethereum,
    Solana,
}

/// Struct that encapsulates a validation error. It contains the row where the error occurred and the error message.
#[derive(Serialize, Debug)]
pub struct ValidationError {
    pub row: usize,
    pub message: String,
}

/// Checks if a string is a valid Ethereum address.
///
/// # Examples
///
/// ```
/// use sablier_merkle_api::utils::csv_validator::is_valid_eth_address;
///
/// let valid_address = "0xf31b00e025584486f7c37Cf0AE0073c97c12c634";
/// let invalid_address = "0xthisIsNotAnAddress";
/// let valid_response = is_valid_eth_address(valid_address);
/// let invalid_response = is_valid_eth_address(invalid_address);
///
/// assert!(valid_response);
/// assert!(!invalid_response);
/// ```
pub fn is_valid_eth_address(address: &str) -> bool {
    Address::try_from(address).is_ok()
}

/// Checks if a string is a valid Solana address.
///
/// # Examples
///
/// ```
/// use sablier_merkle_api::utils::csv_validator::is_valid_sol_address;
///
/// let valid_address = "9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y";
/// let invalid_address = "0xthisIsNotAnAddress";
/// let valid_response = is_valid_sol_address(valid_address);
/// let invalid_response = is_valid_sol_address(invalid_address);
///
/// assert!(valid_response);
/// assert!(!invalid_response);
/// ```
pub fn is_valid_sol_address(address: &str) -> bool {
    Pubkey::from_str(address).is_ok()
}

/// Generic trait for a CSV column validator.
pub trait ColumnValidator {
    /// Generic function that validates a CSV cell.
    fn validate_cel(&self, cel: &str, row_index: usize) -> Option<ValidationError>;
    /// Generic function that validates a csv header.
    fn validate_header(&self, cel: &str) -> Option<ValidationError>;
}

/// Validator for a csv column that should contain valid blockchain addresses
pub struct AddressColumnValidator {
    pub address_type: AddressType,
}

impl AddressColumnValidator {
    /// Creates a new AddressColumnValidator with a specific address type
    pub fn new(address_type: AddressType) -> Self {
        Self { address_type }
    }
}

impl ColumnValidator for AddressColumnValidator {
    /// Validate if a CSV cell contains a valid blockchain address
    ///
    ///  # Examples
    ///
    /// ```
    /// use sablier_merkle_api::utils::csv_validator::{AddressColumnValidator, ColumnValidator, AddressType};
    /// let valid_eth_address = "0xf31b00e025584486f7c37Cf0AE0073c97c12c634";
    /// let valid_sol_address = "9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y";
    /// let invalid_address = "0xthisIsNotAnAddress";
    ///
    /// let eth_validator = AddressColumnValidator::ethereum();
    /// let sol_validator = AddressColumnValidator::solana();
    ///
    /// let result_valid_eth = eth_validator.validate_cel(valid_eth_address, 0);
    /// let result_valid_sol = sol_validator.validate_cel(valid_sol_address, 0);
    /// let result_invalid_eth = eth_validator.validate_cel(invalid_address, 0);
    /// let result_invalid_sol = sol_validator.validate_cel(invalid_address, 0);
    ///
    /// assert!(result_valid_eth.is_none());
    /// assert!(result_valid_sol.is_none());
    /// assert!(!result_invalid_eth.is_none());
    /// assert!(!result_invalid_sol.is_none());
    /// ```
    fn validate_cel(&self, cel: &str, row_index: usize) -> Option<ValidationError> {
        let is_valid = match self.address_type {
            AddressType::Ethereum => is_valid_eth_address(cel),
            AddressType::Solana => is_valid_sol_address(cel),
        };

        if !is_valid {
            let error_message = match self.address_type {
                AddressType::Ethereum => "Invalid Ethereum address",
                AddressType::Solana => "Invalid Solana address",
            };
            return Some(ValidationError { row: row_index + 2, message: String::from(error_message) });
        }
        None
    }

    /// Validate if the csv header is valid
    ///     
    ///  # Examples
    ///
    /// ```
    /// use sablier_merkle_api::utils::csv_validator::{AddressColumnValidator, ColumnValidator};
    /// let eth_validator = AddressColumnValidator::ethereum();
    /// let sol_validator = AddressColumnValidator::solana();
    ///
    /// let result_valid_eth = eth_validator.validate_header("address");
    /// let result_valid_sol = sol_validator.validate_header("address");
    /// let result_invalid = eth_validator.validate_header("amount");
    ///
    /// assert!(result_valid_eth.is_none());
    /// assert!(result_valid_sol.is_none());
    /// assert!(!result_invalid.is_none());
    /// ```
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

/// Validator for a csv column that should contain valid amount. The format of the amount is determined through the
/// regex var.
pub struct AmountColumnValidator {
    pub regex: Regex,
}

impl ColumnValidator for AmountColumnValidator {
    /// Validate if a CSV cell contains a valid amount
    ///
    ///  # Examples
    ///
    /// ```
    /// use sablier_merkle_api::utils::csv_validator::{AmountColumnValidator, ColumnValidator};
    /// use regex::Regex;
    ///
    /// let amount_regex = Regex::new(r"^[+]?\d*\.?\d{0,3}$").unwrap();
    /// let amount_validator = AmountColumnValidator { regex: amount_regex };
    /// let valid_amount = "22.0";
    /// let alphanumeric_amount = "thisIsNotAnAmount";
    /// let zero_amount = "0";
    /// let negative_amount = "-1";
    ///
    /// let result_valid = amount_validator.validate_cel(valid_amount, 0);
    /// let result_alpha = amount_validator.validate_cel(alphanumeric_amount, 0);
    /// let result_zero = amount_validator.validate_cel(zero_amount, 0);
    /// let result_negative = amount_validator.validate_cel(negative_amount, 0);
    ///
    /// assert!(result_valid.is_none());
    /// assert!(!result_alpha.is_none());
    /// assert!(!result_zero.is_none());
    /// assert!(!result_negative.is_none());
    /// ```
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

    /// Validate if the csv header is valid
    ///
    ///  # Examples
    ///
    /// ```
    /// use sablier_merkle_api::utils::csv_validator::{AmountColumnValidator, ColumnValidator};
    /// use regex::Regex;
    ///
    /// let amount_regex = Regex::new(r"^[+]?\d*\.?\d{0,3}$").unwrap();
    /// let amount_validator = AmountColumnValidator { regex: amount_regex };
    /// let result_valid = amount_validator.validate_header("amount");
    /// let result_invalid = amount_validator.validate_header("address");
    ///
    /// assert!(result_valid.is_none());
    /// assert!(!result_invalid.is_none());
    /// ```
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

/// Validates a full CSV row based on an array of objects that implement the ColumnValidator trait.
///
///  # Examples
/// ```
/// 
/// use sablier_merkle_api::utils::csv_validator::{AddressColumnValidator ,AmountColumnValidator, ColumnValidator,validate_csv_row};
/// use regex::Regex;
/// use csv::StringRecord;
///
/// const VALID_ETH_ADDRESS: &str = "0xf31b00e025584486f7c37Cf0AE0073c97c12c634";
/// const INVALID_ETH_ADDRESS: &str = "0xthisIsNotAnAddress";
/// let eth_address_validator = AddressColumnValidator::ethereum();
/// let amount_regex = Regex::new(r"^[+]?\d*\.?\d{0,3}$").unwrap();
/// let amount_validator = AmountColumnValidator { regex: amount_regex };
/// let validators: Vec<&dyn ColumnValidator> = vec![&eth_address_validator, &amount_validator];
/// let valid_row = StringRecord::from(vec![VALID_ETH_ADDRESS, "489.312"]);
/// assert!(validate_csv_row(&valid_row, 0, &validators).is_empty());
/// let insufficient_columns: StringRecord = StringRecord::from(vec![VALID_ETH_ADDRESS]);
/// assert!(!validate_csv_row(&insufficient_columns, 0, &validators).is_empty());
/// let invalid_address = StringRecord::from(vec!["thisIsNotAnAddress", "12534"]);
/// assert!(!validate_csv_row(&invalid_address, 0, &validators).is_empty());
/// let invalid_amount = StringRecord::from(vec![VALID_ETH_ADDRESS, "12.576757"]);
/// assert!(!validate_csv_row(&invalid_amount, 0, &validators).is_empty());
///
/// const VALID_SOLANA_ADDRESS: &str = "9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y";
/// const INVALID_SOLANA_ADDRESS: &str = "0xthisIsNotAnAddress";
/// let sol_address_validator = AddressColumnValidator::solana();
/// let validators: Vec<&dyn ColumnValidator> = vec![&sol_address_validator, &amount_validator];
/// let valid_row = StringRecord::from(vec![VALID_SOLANA_ADDRESS, "489.312"]);
/// assert!(validate_csv_row(&valid_row, 0, &validators).is_empty());
/// let invalid_address = StringRecord::from(vec![INVALID_SOLANA_ADDRESS, "12534"]);
/// assert!(!validate_csv_row(&invalid_address, 0, &validators).is_empty());
/// let invalid_amount = StringRecord::from(vec![VALID_SOLANA_ADDRESS, "12.576757"]);
/// assert!(!validate_csv_row(&invalid_amount, 0, &validators).is_empty());
///  ```
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

/// Validates a full CSV header based on an array of objects that implement the ColumnValidator trait.
///
///  # Examples
/// ```
/// 
/// use sablier_merkle_api::utils::csv_validator::{AddressColumnValidator ,AmountColumnValidator, ColumnValidator,validate_csv_header};
/// use regex::Regex;
/// use csv::StringRecord;
///
/// let address_validator = AddressColumnValidator::ethereum();
/// let amount_regex = Regex::new(r"^[+]?\d*\.?\d{0,3}$").unwrap();
/// let amount_validator = AmountColumnValidator { regex: amount_regex };
/// let validators: Vec<&dyn ColumnValidator> = vec![&address_validator, &amount_validator];
/// let valid_header = StringRecord::from(vec!["address", "amount"]);
/// assert!(validate_csv_header(&valid_header, &validators).is_none());
/// let invalid_address_header = StringRecord::from(vec!["address_invalid", "amount"]);
/// assert!(validate_csv_header(&invalid_address_header, &validators).is_some());
/// let invalid_amount_header = StringRecord::from(vec!["address", "amount_invalid"]);
/// assert!(validate_csv_header(&invalid_amount_header, &validators).is_some());
///  ```
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
    const VALID_SOL_ADDRESS: &str = "9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y";
    const INVALID_SOL_ADDRESS: &str = "0xthisIsNotAnAddress";
    const AMOUNT_PATTERN: &str = r"^[+]?\d*\.?\d{0,3}$";

    fn create_validators() -> (AddressColumnValidator, AddressColumnValidator, AmountColumnValidator) {
        let eth_address_validator = AddressColumnValidator::new(AddressType::Ethereum);
        let sol_address_validator = AddressColumnValidator::new(AddressType::Solana);

        let amount_regex = Regex::new(AMOUNT_PATTERN).unwrap();
        let amount_validator = AmountColumnValidator { regex: amount_regex };
        (eth_address_validator, sol_address_validator, amount_validator)
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
    fn sol_address_validation() {
        assert!(is_valid_sol_address(VALID_SOL_ADDRESS));
        assert!(!is_valid_sol_address(INVALID_SOL_ADDRESS));
    }

    #[test]
    fn eth_address_column_validator_tests() {
        let (eth_address_validator, _, _) = create_validators();
        assert_validation_cel(&eth_address_validator, VALID_ETH_ADDRESS, true);
        assert_validation_cel(&eth_address_validator, INVALID_ETH_ADDRESS, false);
        assert_validation_header(&eth_address_validator, "address", true);
        assert_validation_header(&eth_address_validator, "amount", false);
    }

    #[test]
    fn sol_address_column_validator_tests() {
        let (_, sol_address_validator, _) = create_validators();

        assert_validation_cel(&sol_address_validator, VALID_SOL_ADDRESS, true);
        assert_validation_cel(&sol_address_validator, INVALID_SOL_ADDRESS, false);
        assert_validation_header(&sol_address_validator, "address", true);
        assert_validation_header(&sol_address_validator, "amount", false);
    }

    #[test]
    fn amount_column_validator_tests() {
        let (_, _, amount_validator) = create_validators();
        assert_validation_cel(&amount_validator, "123.45", true);
        assert_validation_cel(&amount_validator, "thisIsNotANumber", false);
        assert_validation_cel(&amount_validator, "0.0", false);
        assert_validation_header(&amount_validator, "amount", true);
        assert_validation_header(&amount_validator, "address", false);
    }

    #[test]
    fn csv_row_validation() {
        let (eth_address_validator, sol_address_validator, amount_validator) = create_validators();
        let eth_validators: Vec<&dyn ColumnValidator> = vec![&eth_address_validator, &amount_validator];
        let sol_validators: Vec<&dyn ColumnValidator> = vec![&sol_address_validator, &amount_validator];

        let valid_eth_row = StringRecord::from(vec![VALID_ETH_ADDRESS, "489.312"]);
        assert!(validate_csv_row(&valid_eth_row, 0, &eth_validators).is_empty());

        let insufficient_columns: StringRecord = StringRecord::from(vec![VALID_ETH_ADDRESS]);
        assert!(!validate_csv_row(&insufficient_columns, 0, &eth_validators).is_empty());

        let invalid_address = StringRecord::from(vec!["thisIsNotAnAddress", "12534"]);
        assert!(!validate_csv_row(&invalid_address, 0, &eth_validators).is_empty());

        let invalid_amount = StringRecord::from(vec![VALID_ETH_ADDRESS, "12.576757"]);
        assert!(!validate_csv_row(&invalid_amount, 0, &eth_validators).is_empty());

        let valid_sol_row = StringRecord::from(vec![VALID_SOL_ADDRESS, "489.312"]);
        assert!(validate_csv_row(&valid_sol_row, 0, &sol_validators).is_empty());

        let invalid_address = StringRecord::from(vec![INVALID_SOL_ADDRESS, "12534"]);
        assert!(!validate_csv_row(&invalid_address, 0, &sol_validators).is_empty());

        let invalid_amount = StringRecord::from(vec![VALID_SOL_ADDRESS, "12.576757"]);
        assert!(!validate_csv_row(&invalid_amount, 0, &sol_validators).is_empty());
    }

    #[test]
    fn csv_header_validation() {
        let (address_validator, _, amount_validator) = create_validators();
        let validators: Vec<&dyn ColumnValidator> = vec![&address_validator, &amount_validator];

        let valid_header = StringRecord::from(vec!["address", "amount"]);
        assert!(validate_csv_header(&valid_header, &validators).is_none());

        let invalid_address_header = StringRecord::from(vec!["address_invalid", "amount"]);
        assert!(validate_csv_header(&invalid_address_header, &validators).is_some());

        let invalid_amount_header = StringRecord::from(vec!["address", "amount_invalid"]);
        assert!(validate_csv_header(&invalid_amount_header, &validators).is_some());
    }
}
