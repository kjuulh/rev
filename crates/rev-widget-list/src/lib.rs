use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};
pub use widget::{WidgetList, WidgetListItem, WidgetListState};

/// [`SelectableWidgetList`] is a convenience method for [`WidgetList`].
/// It provides the next and previous method to select items and it
/// implements the [`Widget`] trait.
#[derive(Clone, Default)]
pub struct SelectableWidgetList<'a, T> {
    /// Holds the lists state, i.e. which element is selected.
    pub state: WidgetListState,

    /// The list of widgets.
    pub items: Vec<T>,

    /// Style used as a base style for the widget.
    style: Style,

    /// Block surrounding the widget list.
    block: Option<Block<'a>>,

    /// Whether the selection is circular. If true, calling next on the
    /// last element returns the first element, and calling previous on
    /// the first element returns the last element.
    circular: bool,

    /// If truncate is true, the widget list will render over the full-screen.
    /// In this case the first/last widget might be truncated.
    truncate: bool,
}

impl<'a, T> SelectableWidgetList<'a, T>
where
    T: Widget + Into<WidgetListItem<T>> + Clone,
{
    /// `items` must implement [`Widget`] and should be castable into [`WidgetListItem`].
    #[must_use]
    pub fn new(items: Vec<T>) -> Self {
        Self {
            state: WidgetListState::default(),
            items,
            style: Style::default(),
            block: None,
            circular: true,
            truncate: true,
        }
    }

    /// The base style of the list. Not the style of the list elements.
    #[must_use]
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// The base block around the list. Must not be set.
    #[must_use]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set circular selection. When circular is True, the selection continues
    /// from the last item to the first, and vice versa.
    #[must_use]
    pub fn circular(mut self, circular: bool) -> Self {
        self.circular = circular;
        self
    }

    /// If truncate is true, the widget list will render over the full-screen.
    /// In this case the first/last widget might be truncated.
    #[must_use]
    pub fn truncate(mut self, truncate: bool) -> Self {
        self.truncate = truncate;
        self
    }

    /// Selects the next element of the list. If circular is true,
    /// calling next on the last element selects the first.
    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    if self.circular {
                        0
                    } else {
                        i
                    }
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Selects the previous element of the list. If circular is true,
    /// calling previous on the first element selects the last.
    pub fn previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    if self.circular {
                        self.items.len() - 1
                    } else {
                        i
                    }
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Returns a reference to the selected item.
    #[must_use]
    pub fn get_selected(&self) -> Option<&T> {
        self.state
            .selected()
            .and_then(|index| self.items.get(index))
    }

    /// Returns a mutable reference to the selected item.
    #[must_use]
    pub fn get_selected_mut(&mut self) -> Option<&mut T> {
        self.state
            .selected()
            .and_then(|index| self.items.get_mut(index))
    }
}

impl<T> Widget for &mut SelectableWidgetList<'_, T>
where
    T: Widget + Into<WidgetListItem<T>> + Clone,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items = self.items.iter().map(|x| x.clone().into()).collect();
        let mut widget = WidgetList::new(items)
            .style(self.style)
            .truncate(self.truncate);
        if let Some(block) = self.block.clone() {
            widget = widget.block(block);
        }

        widget.render(area, buf, &mut self.state);
    }
}

pub mod widget {

    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::Style,
        widgets::{Block, StatefulWidget, Widget},
    };

    #[derive(Debug, Clone, Default)]
    pub struct WidgetListState {
        /// The index of the fist item on the screen
        offset: usize,

        /// The selected item
        selected: Option<usize>,
    }

    impl WidgetListState {
        /// Return the currently selected items index
        #[must_use]
        pub fn selected(&self) -> Option<usize> {
            self.selected
        }

        /// Select an item by its index
        pub fn select(&mut self, index: Option<usize>) {
            self.selected = index;
            if index.is_none() {
                self.offset = 0;
            }
        }

        /// Here we check and if necessary update the viewport. For this we start with the first item
        /// on the screen and iterate until we have reached the maximum height. If the selected value
        /// is within the bounds we do nothing. If the selected value is out of bounds, we adjust the
        /// offset accordingly.
        fn update_view_port(
            &mut self,
            heights: &[u16],
            max_height: u16,
            truncate: bool,
        ) -> Vec<u16> {
            // The items heights on the viewport will be calculated on the fly.
            let mut view_heights: Vec<u16> = Vec::new();

            // Select the first element if none is selected
            let selected = self.selected.unwrap_or(0);

            // If the selected value is smaller than the offset, we roll
            // the offset so that the selected value is at the top
            if selected < self.offset {
                self.offset = selected;
            }

            // Check if the selected item is in the current view
            let (mut y, mut i) = (0, self.offset);
            let mut found = false;
            for height in heights.iter().skip(self.offset) {
                // Out of bounds
                if y + height > max_height {
                    if truncate {
                        // Truncate the last widget
                        view_heights.push(max_height - y);
                    }
                    break;
                }
                // Selected value is within view/bounds, so we are good
                // but we keep iterating to collect the view heights
                if selected == i {
                    found = true;
                }
                y += height;
                i += 1;
                view_heights.push(*height);
            }
            if found {
                return view_heights;
            }

            // The selected item is out of bounds. We iterate backwards from the selected
            // item and determine the first widget that still fits on the screen.
            view_heights.clear();
            let (mut y, mut i) = (0, selected);
            let last_elem = heights.len().saturating_sub(1);
            for height in heights.iter().rev().skip(last_elem - selected) {
                // out of bounds
                if y + height >= max_height {
                    if truncate {
                        // Truncate the first widget.
                        // At the moment this will truncate the bottom of the first item, which
                        // looks a bit strange, but I have not figured out how to truncate a
                        // widget from the top.
                        view_heights.insert(0, max_height - y);
                        self.offset = i;
                    } else {
                        self.offset = i + 1;
                    }
                    break;
                }
                view_heights.insert(0, *height);
                y += height;
                i -= 1;
            }
            view_heights
        }
    }

    /// `WidgetListItem` holds the widget and the height of the widget.
    #[derive(Clone)]
    pub struct WidgetListItem<T> {
        /// The widget.
        pub content: T,

        /// The height of the widget.
        pub height: u16,

        /// A callback function that can be used to style an item
        /// based on its selection state.
        modify_fn: ModifyFn<Self>,
    }

    impl<T: Widget> WidgetListItem<T> {
        /// Constructs a new item given the widget and its height
        pub fn new(content: T, height: u16) -> Self {
            Self {
                content,
                height,
                modify_fn: default_modify_fn,
            }
        }

        /// Set a callback that can be used to modify the widget item
        /// based on the selection state.
        #[must_use]
        pub fn modify_fn(mut self, modify_fn: ModifyFn<Self>) -> Self {
            self.modify_fn = modify_fn;
            self
        }
    }

    impl<T: Widget> Widget for WidgetListItem<T> {
        fn render(self, area: Rect, buf: &mut Buffer) {
            self.content.render(area, buf);
        }
    }

    /// `ModifyFn` is a callback function that takes in the widget
    /// and the current selection state and returns the (modified)
    /// widget.
    ///
    /// A selection state of None indicates that no other element
    /// is selected. If the selection state is true, it indicates
    /// that the current item is selected.
    pub type ModifyFn<T> = fn(T, Option<bool>) -> T;

    /// Default implementation of `modify_fn`. Does nothing to T.
    fn default_modify_fn<T>(slf: T, _: Option<bool>) -> T {
        slf
    }

    #[derive(Clone)]
    pub struct WidgetList<'a, T> {
        /// The lists items
        items: Vec<WidgetListItem<T>>,

        /// Style used as a base style for the widget
        style: Style,

        /// Block surrounding the widget list
        block: Option<Block<'a>>,

        /// Truncate widgets to fill full screen. Defaults to true.
        truncate: bool,
    }

    impl<'a, T> Default for WidgetList<'a, T> {
        fn default() -> Self {
            Self {
                items: vec![],
                style: Style::default(),
                block: None,
                truncate: true,
            }
        }
    }

    impl<'a, T: Widget> WidgetList<'a, T> {
        /// Instantiate a widget list with elements. The Elements must
        /// implement the [`Widget`] trait.
        #[must_use]
        pub fn new(items: Vec<WidgetListItem<T>>) -> Self {
            Self {
                items,
                style: Style::default(),
                block: None,
                truncate: true,
            }
        }

        /// Set the block style which surrounds the whole List.
        #[must_use]
        pub fn block(mut self, block: Block<'a>) -> Self {
            self.block = Some(block);
            self
        }

        /// Set the base style of the List.
        #[must_use]
        pub fn style(mut self, style: Style) -> Self {
            self.style = style;
            self
        }

        /// If truncate is true, the list fills the full screen
        /// and truncates the first or last item of the list.
        /// It is true by default.
        #[must_use]
        pub fn truncate(mut self, truncate: bool) -> Self {
            self.truncate = truncate;
            self
        }

        /// Whether the widget list is empty
        #[must_use]
        pub fn is_empty(&self) -> bool {
            self.items.is_empty()
        }

        /// Returns the length of the widget list
        #[must_use]
        pub fn len(&self) -> usize {
            self.items.len()
        }
    }

    impl<'a, T: Widget> StatefulWidget for WidgetList<'a, T> {
        type State = WidgetListState;

        fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
            // Set the base style
            buf.set_style(area, self.style);
            let area = match self.block.take() {
                Some(b) => {
                    let inner_area = b.inner(area);
                    b.render(area, buf);
                    inner_area
                }
                None => area,
            };

            // List is empty
            if self.is_empty() {
                return;
            }

            // Use the full width
            let width = area.width;

            // Maximum height
            let max_height = area.height;

            // The starting positions of the current item
            let x = area.left();
            let y0 = area.top();
            let mut y = y0;

            // Modify the widgets based on their selection state. Split out their heights for
            // efficiency as we have to iterate over the heights back and forth to determine
            // which widget is shown on the viewport.
            let (raw_heights, modified_items): (Vec<_>, Vec<_>) = self
                .items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    let is_selected = state.selected().map(|selected| selected == i);
                    let item = (item.modify_fn)(item, is_selected);
                    (item.height, item)
                })
                .unzip();

            // Determine which widgets to render and how much space they are assigned to.
            let view_heights = state.update_view_port(&raw_heights, max_height, self.truncate);

            // Iterate over all items
            let first = state.offset;
            let n = view_heights.len();
            for (i, item) in modified_items.into_iter().skip(first).take(n).enumerate() {
                // Set the drawing area of the current item
                let height = view_heights[i];
                let area = Rect::new(x, y, width, height);

                // Render the item
                let is_selected = state.selected().map(|selected| selected == i + first);
                (item.modify_fn)(item, is_selected).render(area, buf);

                // Update the offset
                y += height;
            }
        }
    }
}
