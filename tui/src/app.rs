use crate::components::menu::CurrentMenu;

pub enum Message {
    SwitchMenu(CurrentMenu),
}
