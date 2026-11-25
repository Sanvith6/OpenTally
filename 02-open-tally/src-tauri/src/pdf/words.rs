use rust_decimal::Decimal;

const ONES: [&str; 20] = [
    "",
    "One",
    "Two",
    "Three",
    "Four",
    "Five",
    "Six",
    "Seven",
    "Eight",
    "Nine",
    "Ten",
    "Eleven",
    "Twelve",
    "Thirteen",
    "Fourteen",
    "Fifteen",
    "Sixteen",
    "Seventeen",
    "Eighteen",
    "Nineteen",
];

const TENS: [&str; 10] = [
    "", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety",
];

fn two_digits_to_words(n: u64) -> String {
    if n < 20 {
        ONES[n as usize].to_string()
    } else {
        let ten = TENS[(n / 10) as usize];
        let one = ONES[(n % 10) as usize];
        if one.is_empty() {
            ten.to_string()
        } else {
            format!("{} {}", ten, one)
        }
    }
}

fn three_digits_to_words(n: u64) -> String {
    let hundred = n / 100;
    let rem = n % 100;
    if hundred > 0 && rem > 0 {
        format!(
            "{} Hundred {}",
            ONES[hundred as usize],
            two_digits_to_words(rem)
        )
    } else if hundred > 0 {
        format!("{} Hundred", ONES[hundred as usize])
    } else {
        two_digits_to_words(rem)
    }
}

/// Converts a financial amount into words in the Indian numbering system.
/// Uses exact rust_decimal integer math with zero floating-point arithmetic.
///
/// Example:
/// 125000.50 -> "Rupees One Lakh Twenty Five Thousand and Fifty Paise Only"
/// 10000000 -> "Rupees One Crore Only"
pub fn number_to_indian_words(amount: Decimal) -> String {
    if amount == Decimal::ZERO {
        return "Rupees Zero Only".to_string();
    }

    let is_negative = amount.is_sign_negative();
    let abs_amount = amount.abs();

    // Integer rupees and fractional paise
    let rupees_dec = abs_amount.trunc();
    let paise_dec = (abs_amount.fract() * Decimal::new(100, 0)).trunc();

    let rupees = rupees_dec.to_string().parse::<u64>().unwrap_or(0);
    let paise = paise_dec.to_string().parse::<u64>().unwrap_or(0);

    let mut parts = Vec::new();

    if rupees == 0 {
        parts.push("Zero".to_string());
    } else {
        // Indian numbering groups:
        // Crores: >= 1,00,00,000
        // Lakhs:  >= 1,00,000
        // Thousands: >= 1,000
        // Hundreds/Tens/Units: < 1,000
        let mut rem = rupees;

        let crores = rem / 10_000_000;
        rem %= 10_000_000;

        let lakhs = rem / 100_000;
        rem %= 100_000;

        let thousands = rem / 1_000;
        rem %= 1_000;

        let hundreds_units = rem;

        if crores > 0 {
            if crores >= 100 {
                parts.push(format!("{} Crore", three_digits_to_words(crores)));
            } else {
                parts.push(format!("{} Crore", two_digits_to_words(crores)));
            }
        }

        if lakhs > 0 {
            parts.push(format!("{} Lakh", two_digits_to_words(lakhs)));
        }

        if thousands > 0 {
            parts.push(format!("{} Thousand", two_digits_to_words(thousands)));
        }

        if hundreds_units > 0 {
            parts.push(three_digits_to_words(hundreds_units));
        }
    }

    let rupees_str = parts.join(" ");

    let result = if rupees == 0 && paise > 0 {
        let paise_words = two_digits_to_words(paise);
        format!("{} Paise Only", paise_words)
    } else if paise > 0 {
        let paise_words = two_digits_to_words(paise);
        format!("Rupees {} and {} Paise Only", rupees_str, paise_words)
    } else {
        format!("Rupees {} Only", rupees_str)
    };

    if is_negative {
        format!("Minus {}", result)
    } else {
        result
    }
}

/// Formats an exact Decimal amount in standard Indian comma notation:
/// 1234567.89 -> "12,34,567.89"
pub fn format_indian_currency(amount: Decimal) -> String {
    let sign = if amount.is_sign_negative() { "-" } else { "" };
    let abs_val = amount.abs();
    let int_part = abs_val.trunc().to_string();
    let fract_part = format!("{:.2}", abs_val.fract())
        .trim_start_matches('0')
        .to_string();

    let chars: Vec<char> = int_part.chars().collect();
    let n = chars.len();

    let formatted = if n <= 3 {
        int_part
    } else {
        let last_three = &int_part[n - 3..];
        let mut rest = &int_part[..n - 3];
        let mut groups = Vec::new();

        while !rest.is_empty() {
            if rest.len() <= 2 {
                groups.push(rest.to_string());
                break;
            } else {
                let chunk = &rest[rest.len() - 2..];
                groups.push(chunk.to_string());
                rest = &rest[..rest.len() - 2];
            }
        }
        groups.reverse();
        format!("{},{}", groups.join(","), last_three)
    };

    format!("{sign}{formatted}{fract_part}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_indian_words_cases() {
        assert_eq!(number_to_indian_words(Decimal::ZERO), "Rupees Zero Only");
        assert_eq!(
            number_to_indian_words(Decimal::from_str("125000.50").unwrap()),
            "Rupees One Lakh Twenty Five Thousand and Fifty Paise Only"
        );
        assert_eq!(
            number_to_indian_words(Decimal::from_str("10000000").unwrap()),
            "Rupees One Crore Only"
        );
        assert_eq!(
            number_to_indian_words(Decimal::from_str("10000000.00").unwrap()),
            "Rupees One Crore Only"
        );
        assert_eq!(
            number_to_indian_words(Decimal::from_str("45231.25").unwrap()),
            "Rupees Forty Five Thousand Two Hundred Thirty One and Twenty Five Paise Only"
        );
    }

    #[test]
    fn test_indian_currency_formatting() {
        assert_eq!(format_indian_currency(Decimal::ZERO), "0.00");
        assert_eq!(
            format_indian_currency(Decimal::from_str("125000.50").unwrap()),
            "1,25,000.50"
        );
        assert_eq!(
            format_indian_currency(Decimal::from_str("10000000").unwrap()),
            "1,00,00,000.00"
        );
    }
}
