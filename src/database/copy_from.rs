use itertools::Itertools;

pub fn format_binary_array(value: &Vec<Vec<u8>>) -> String {
    if value.len() == 0 {
        return "\\N".to_string(); // A null-value takes less space than an empty array
    }
    format!("{{{}}}", value.iter().map(|w| format_binary(w)).join(","))
}

pub fn format_binary(value: &Vec<u8>) -> String {
    format!("\\\\x{}", hex::encode(value))
}
