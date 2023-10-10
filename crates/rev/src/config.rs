use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;

#[derive(Debug, Clone)]
#[derive(Default)]
pub struct Config {
    pub keybinds: Keybinds,
}



pub type InnerKeybinds = HashMap<Vec<KeyEvent>, Action>;

#[derive(Clone, Debug)]
pub struct Keybinds(pub InnerKeybinds);

impl Deref for Keybinds {
    type Target = InnerKeybinds;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Keybinds {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for Keybinds {
    fn default() -> Self {
        let mut keybinds = HashMap::new();
        keybinds.insert(vec![parse_key_event("q").unwrap()], Action::Quit);

        Self(keybinds)
    }
}

fn parse_key_event(raw: &str) -> anyhow::Result<KeyEvent> {
    let raw_lower = raw.to_ascii_lowercase();

    let e = match &raw_lower {
        c if c.len() == 1 => {
            let c = c.chars().next().expect("to get next key code");
            KeyCode::Char(c)
        }
        _ => anyhow::bail!("Unable to parse {raw_lower}"),
    };

    Ok(KeyEvent::new(e, KeyModifiers::empty()))
}
