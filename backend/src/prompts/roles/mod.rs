// Root and child managers receive the same end-node protocol. Keep their
// legacy wording distinct here: only the child hands off to a parent manager.
pub(super) fn manager_end_node_contract(child: bool) -> &'static str {
    if child {
        "An explicit end_node waits for all other nodes completed. Successful dispatch hands its open Chat to the user without report or complete; explicitly finish this child Run when canFinish=true, then report normally to the parent manager. Failed dispatch cannot finish."
    } else {
        "Map may explicitly mark one end_node. It must wait for every other node to be completed. After successful dispatch, that Chat is handed to the user and remains running/open: do not request reports or complete it. Explicitly finish the Run when canFinish is true; this records handed-to-user, not completion of that Chat. Never finish after a failed dispatch."
    }
}

pub(super) const NODE_APPROVAL: &str = "Approve the current session result before the final handoff report. Approval and reporting are independent; progress, problems and failures may be reported at any time without sending confirmation. Approval never completes work or notifies the manager; the manager must explicitly complete after a latest ready report and declared deliveries. Use the native approval tool; manual mode requires explicit native Yes.";
