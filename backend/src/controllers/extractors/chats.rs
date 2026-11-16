use super::CommandAction;

pub(crate) struct PreviewChat;
impl CommandAction for PreviewChat {
    const ACTION: &'static str = "Chat 名称预览";
}

pub(crate) struct CreateChat;
impl CommandAction for CreateChat {
    const ACTION: &'static str = "创建 Chat";
}
