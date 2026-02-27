use std::collections::HashMap;

/// A cache implementation
pub struct Cache {
    items: HashMap<String, String>,
    max_size: usize,
}

impl Cache {
    pub fn new(max_size: usize) -> Self {
        Cache {
            items: HashMap::new(),
            max_size,
        }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.items.get(key)
    }

    pub fn set(&mut self, key: String, value: String) -> bool {
        if self.items.len() >= self.max_size {
            return false;
        }
        self.items.insert(key, value);
        true
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.items.remove(key).is_some()
    }
}

/// A different type with a method named "get"
pub struct Registry {
    entries: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            entries: Vec::new(),
        }
    }

    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }

    pub fn register(&mut self, entry: String) {
        self.entries.push(entry);
    }
}

/// Top-level function
pub fn get(key: &str) -> Option<String> {
    Some(key.to_string())
}
