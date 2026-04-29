/// Semantic role for text styling (backend sends semantics, frontend maps to theme).
#[derive(Debug, Clone, PartialEq)]
pub enum TextRole {
    Label,
    Body,
    Title,
}

/// Declarative UI commands produced by the ViewModel.
/// The frontend translates these into egui draw calls.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewCommand {
    /// Vertical stack of children.
    VStack { children: Vec<ViewCommand> },
    /// Horizontal stack of children.
    HStack { children: Vec<ViewCommand> },
    /// Static text label.
    Text {
        content: String,
        role: TextRole,
        size: f32,
    },
    /// Single-line text input.
    TextInput {
        id: String,
        value: String,
        placeholder: String,
        password: bool,
        width: f32,
    },
    /// Dropdown selector.
    ComboBox {
        id: String,
        selected: String,
        options: Vec<String>,
        width: f32,
    },
    /// Clickable button.
    Button {
        id: String,
        label: String,
        min_width: f32,
        min_height: f32,
    },
    /// Vertical spacer.
    Space { height: f32 },
}

/// User interaction events captured by the frontend and sent to the ViewModel.
#[derive(Debug, Clone, PartialEq)]
pub enum UserAction {
    TextInputChange { id: String, value: String },
    ComboChange { id: String, selected: String },
    ButtonClick { id: String },
}
