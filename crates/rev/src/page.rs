use crate::{components::Component, tui::Frame};

pub struct Page {
    name: String,
    components: Vec<Box<dyn Component>>,
}

impl Page {
    pub fn new(name: impl Into<String>, components: Vec<Box<dyn Component>>) -> Self {
        Self {
            name: name.into(),
            components,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn get_components(&mut self) -> &mut [Box<dyn Component>] {
        &mut self.components
    }

    pub fn apply(
        &mut self,
        apply_fn: impl Fn(&mut Box<dyn Component>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let components = self.get_components();
        for c in components.iter_mut() {
            apply_fn(c)?;
        }

        Ok(())
    }
    pub fn draw(&mut self, frame: &mut Frame<'_>) -> anyhow::Result<()> {
        let components = self.get_components();

        for c in components.iter_mut() {
            c.draw(frame, frame.size())?;
        }

        Ok(())
    }
}
