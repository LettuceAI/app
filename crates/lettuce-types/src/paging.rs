use serde::{Deserialize, Serialize};

const DEFAULT_LIMIT: u16 = 50;
const MAX_LIMIT: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageLimit(u16);

impl PageLimit {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(if value == 0 {
            DEFAULT_LIMIT
        } else if value > MAX_LIMIT {
            MAX_LIMIT
        } else {
            value
        })
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for PageLimit {
    fn default() -> Self {
        Self(DEFAULT_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PageRequest {
    pub cursor: Option<String>,
    pub limit: PageLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

impl<T> Page<T> {
    #[must_use]
    pub fn last(items: Vec<T>) -> Self {
        Self {
            items,
            next_cursor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PageLimit;

    #[test]
    fn page_limit_is_always_bounded() {
        assert_eq!(PageLimit::new(0).get(), 50);
        assert_eq!(PageLimit::new(20).get(), 20);
        assert_eq!(PageLimit::new(u16::MAX).get(), 200);
    }
}
