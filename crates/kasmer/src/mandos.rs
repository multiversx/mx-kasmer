//! Values as the scenario (Mandos) format writes them, the subset the test setup uses.
//! As `kmultiversx.scenario`.

/// `address:<name>`: the name padded with `_` to 32 bytes.
pub fn address(name: &str) -> Vec<u8> {
    padded(name, 0)
}

/// `sc:<name>`: 10 zero bytes (smart contract addresses start with 8, then 2 for the VM type), then
/// the name, padded with `_` to 32 bytes.
pub fn sc_address(name: &str) -> Vec<u8> {
    padded(name, 10)
}

fn padded(name: &str, leading_zeros: usize) -> Vec<u8> {
    let mut a = vec![0u8; leading_zeros];
    a.extend_from_slice(name.as_bytes());
    a.resize(32.max(a.len()), b'_');
    a.truncate(32);
    a
}

/// An unsigned number as a top-level argument: big-endian, no leading zeros; zero is empty.
pub fn uint_bytes(mut be: &[u8]) -> &[u8] {
    while let [0, rest @ ..] = be {
        be = rest;
    }
    be
}

/// A decimal unsigned number, big-endian with no leading zeros. `None` if not only digits.
pub fn from_decimal(digits: &str) -> Option<Vec<u8>> {
    if digits.is_empty() {
        return None;
    }
    let mut n: Vec<u8> = Vec::new();
    for c in digits.bytes() {
        if !c.is_ascii_digit() {
            return None;
        }
        let mut carry = u32::from(c - b'0');
        for b in n.iter_mut().rev() {
            let cur = u32::from(*b) * 10 + carry;
            *b = cur as u8;
            carry = cur >> 8;
        }
        if carry > 0 {
            n.insert(0, carry as u8);
        }
    }
    Some(uint_bytes(&n).to_vec())
}

/// The decimal digits of a big-endian unsigned number.
pub fn to_decimal(be: &[u8]) -> String {
    let mut n: Vec<u8> = uint_bytes(be).to_vec();
    if n.is_empty() {
        return "0".into();
    }
    let mut digits = Vec::new();
    while !n.is_empty() {
        let mut rem = 0u32;
        for b in n.iter_mut() {
            let cur = (rem << 8) | u32::from(*b);
            *b = (cur / 10) as u8;
            rem = cur % 10;
        }
        digits.push(b'0' + rem as u8);
        let zeros = n.iter().take_while(|&&b| b == 0).count();
        n.drain(..zeros);
    }
    digits.reverse();
    String::from_utf8(digits).expect("digits")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(address("k"), b"k_______________________________");
        let sc = sc_address("k-test");
        assert_eq!(&sc[..10], &[0; 10]);
        assert_eq!(&sc[10..], b"k-test________________");
        assert_eq!(address(&"x".repeat(40)).len(), 32);
    }

    #[test]
    fn decimal() {
        assert_eq!(to_decimal(&[]), "0");
        assert_eq!(to_decimal(&[0, 0]), "0");
        assert_eq!(to_decimal(&[1, 0]), "256");
        assert_eq!(to_decimal(&[0xff; 8]), u64::MAX.to_string());
        assert_eq!(to_decimal(&[0xff; 16]), u128::MAX.to_string());
        assert_eq!(uint_bytes(&[0, 0, 5]), &[5]);
        assert_eq!(from_decimal("0").unwrap(), Vec::<u8>::new());
        assert_eq!(from_decimal("256").unwrap(), vec![1, 0]);
        let max = u128::MAX.to_string();
        assert_eq!(to_decimal(&from_decimal(&max).unwrap()), max);
        assert_eq!(from_decimal("1x"), None);
        assert_eq!(from_decimal(""), None);
    }
}
