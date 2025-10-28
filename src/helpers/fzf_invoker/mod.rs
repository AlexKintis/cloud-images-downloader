use std::fmt::Display;
use termenu::{Item, Menu};

pub struct FzfInvoker<T> {
    msg: String,
    items: Vec<T>,
}

// NOTE: to see the std::fmt::Display
impl<T> FzfInvoker<T>
where
    T: Display + Clone,
{
    pub fn new(msg: String, items: Vec<T>) -> Self {
        Self { msg, items }
    }

    /// Show an fzf-like menu and return the selected item (cloned).
    pub fn invoke(&self) -> Option<T> {
        // Menu::new() -> Result<Menu, io::Error>
        let mut menu = Menu::new().unwrap_or_else(|e| {
            eprintln!("Failed to init menu: {e}");
            std::process::exit(1);
        });

        // Build menu entries
        let mut list: Vec<Item<usize>> = Vec::with_capacity(self.items.len());
        for (idx, item) in self.items.iter().enumerate() {
            list.push(Item::new(&format!("{}", item), idx)); // pass String
        }

        // Show menu and get selected index (&usize)
        let selected_index: &usize = menu
            .set_title(self.msg.as_str())
            .add_list(list)
            .select()
            .unwrap_or_else(|e| {
                eprintln!("Menu error: {e}");
                std::process::exit(1);
            })?; // None if user canceled

        // Use *selected_index (usize) to index the items vec
        self.items.get(*selected_index).cloned()
    }
}
