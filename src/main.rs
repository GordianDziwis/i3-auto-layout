use anyhow::{Error, Result};
use jq_rs::run;
use serde_json::{json, to_string, to_string_pretty, to_value, Value};
use tokio::sync::mpsc;
use tokio_i3ipc::event::{Event, Subscribe, WindowChange};
use tokio_i3ipc::msg::Msg;
use tokio_i3ipc::reply::{Node, NodeLayout, NodeType, Rect};
use tokio_i3ipc::I3;
use tokio_stream::StreamExt;

#[rustfmt::skip]
fn split_rect(r: Rect) -> &'static str {
    if r.width > r.height { "split h" }
    else { "split v" }
}

fn has_splith_workspace_parent(
    node: &Node,
    window_id: usize,
    is_splith_workspace: bool,
    children: i64,
) -> bool {
    if node.id == window_id {
        is_splith_workspace
    } else {
        node.nodes.iter().any(|child| {
            has_splith_workspace_parent(
                child,
                window_id,
                matches!(node.layout, NodeLayout::SplitH)
                    && matches!(node.node_type, NodeType::Workspace)
                    && matches!(node.nodes.len(), children),
                children,
            )
        })
    }
}

fn find_parent(parent: &Node, window_id: usize) -> Option<&Node> {
    if parent.id == window_id {
        None
    } else {
        parent.nodes.iter().find_map(|child| {
            if child.id == window_id {
                Some(parent)
            } else {
                find_parent(child, window_id)
            }
        })
    }
}

fn find_sibling(node: &Node, window_id: usize) -> Option<&Node> {
    if let Some(parent) = find_parent(node, window_id) {
        parent.nodes.iter().find(|&child| child.id != window_id)
    } else {
        None
    }
}

async fn filter_tree(tree: &Node) -> Result<String, String> {
    let json_tree = to_string(tree).unwrap();
    let res = run(
        "def filter_nodes: {layout, name, type, orientation, id, nodes: \
         [.nodes[]? | filter_nodes]}; filter_nodes",
        &json_tree,
    );

    match res {
        Ok(filtered_tree) => {
            let parsed_tree: Value = serde_json::from_str(&filtered_tree)
                .map_err(|e| format!("Error parsing filtered tree: {}", e))?;
            serde_json::to_string_pretty(&parsed_tree)
                .map_err(|e| format!("Error formatting tree: {}", e))
        }
        Err(e) => Err(format!("Error filtering tree: {}", e)),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    flexi_logger::Logger::try_with_str("debug")?.start()?;
    let (send, mut recv) = mpsc::channel::<String>(10);

    let s_handle = tokio::spawn(async move {
        let mut event_listener = {
            let mut i3 = I3::connect().await?;
            i3.subscribe([Subscribe::Window]).await?;
            i3.listen()
        };

        let i3 = &mut I3::connect().await?;

        while let Some(Ok(Event::Window(window_data))) =
            event_listener.next().await
        {
            if matches!(window_data.change, WindowChange::New) {
                let tree = &i3.get_tree().await?;

                let focused_id = window_data.container.id;
                let parent = find_parent(tree, focused_id);
                if let Some(parent) = parent {
                    match parent {
                        Node {
                            nodes,
                            layout: NodeLayout::SplitH,
                            node_type: NodeType::Workspace,
                            ..
                        } if nodes.len() == 3 => {
                            log::debug!("{}", filter_tree(tree).await.unwrap());
                            log::debug!("focused_id: {:#?}", focused_id);
                            // log::debug!(
                            //     "parent: {}",
                            //     filter_tree(parent).await.unwrap()
                            // );
                            let msg = format!(
                                "focus left split v layout tabbed focus right \
                                 move left",
                            );
                            send.send(msg).await?;
                        }
                        _ => (),
                    }
                };
            }
            if matches!(window_data.change, WindowChange::Move) {
                let tree = &i3.get_tree().await?;

                let focused_id = window_data.container.id;
                let parent = find_parent(tree, focused_id);
                if let Some(parent) = parent {
                    match parent {
                        Node {
                            nodes,
                            layout: NodeLayout::SplitH,
                            node_type: NodeType::Workspace,
                            ..
                        } if nodes.len() == 3 && nodes[1].id == focused_id => {
                            log::debug!("{}", filter_tree(tree).await.unwrap());
                            log::debug!("focused_id: {:#?}", focused_id);
                            // log::debug!(
                            //     "parent: {}",
                            //     filter_tree(parent).await.unwrap()
                            // );
                            let msg = format!(
                                "focus left split v layout tabbed focus right \
                                 move left",
                            );
                            send.send(msg).await?;
                        }
                        _ => (),
                    }
                };
            }

            if matches!(window_data.change, WindowChange::Move) {
                let tree = &i3.get_tree().await?;

                let focused_id = window_data.container.id;
                let parent = find_parent(tree, focused_id);
                if let Some(parent) = parent {
                    match parent {
                        Node {
                            nodes,
                            layout: NodeLayout::SplitH,
                            node_type: NodeType::Workspace,
                            ..
                        } if nodes.len() == 2 => {
                            let sibling =
                                find_sibling(tree, focused_id).unwrap();
                            match sibling {
                                Node {
                                    nodes,
                                    layout: NodeLayout::Tabbed,
                                    ..
                                } if nodes.len() == 1 => {
                                    log::debug!(
                                        "{}",
                                        filter_tree(tree).await.unwrap()
                                    );
                                    log::debug!(
                                        "focused_id: {:#?}",
                                        focused_id
                                    );
                                    // log::debug!(
                                    //     "parent: {}",
                                    //     filter_tree(parent).await.unwrap()
                                    // );
                                    let msg = format!(
                                        "[con_id={}] move right",
                                        nodes.first().unwrap().id
                                    );
                                    send.send(msg).await?;
                                }
                                _ => (),
                            };

                            // let msg = format!("[con_id={}] focus", focused_id);
                            // send.send(msg).await?;
                        }
                        _ => (),
                    }
                };
            }
            if window_data.change == WindowChange::Move {
                log::debug!("MOVED");
            }

            // if matches!(
            //     window_data.change,
            //     WindowChange::New | WindowChange::Move
            // ) {
            //     let tree = &i3.get_tree().await?;
            //     log::debug!("{}", filter_tree(tree).await.unwrap());
            //     let is_tabbed = matches!(
            //         window_data.container.layout,
            //         NodeLayout::Tabbed | NodeLayout::Stacked
            //     );

            //     if has_splith_workspace_parent(
            //         tree,
            //         window_data.container.id,
            //         false,
            //         1,
            //     ) & !is_tabbed
            //     {
            //         let focused_id = window_data.container.id;
            //         let msg = format!(
            //             "[con_id={}] split v layout tabbed",
            //             focused_id
            //         );
            //         send.send(msg).await?;
            //         let sibling = find_sibling(tree, focused_id);
            //         if let Some(sibling) = sibling {
            //             if !matches!(
            //                 sibling.layout,
            //                 NodeLayout::Tabbed | NodeLayout::Stacked
            //             ) && sibling.nodes.len() == 0
            //             {
            //                 let msg = format!(
            //                     "[con_id={}] split v layout tabbed",
            //                     sibling.id
            //                 );
            //                 send.send(msg).await?;
            //             }
            //         }
            //     }
            // }
        }
        log::debug!("Sender loop ended");
        Ok::<_, Error>(())
    });

    let r_handle = tokio::spawn(async move {
        let mut i3 = I3::connect().await?;
        while let Some(cmd) = recv.recv().await {
            i3.send_msg_body(Msg::RunCommand, cmd).await?;
        }
        log::debug!("Receiver loop ended");
        Ok::<_, Error>(())
    });

    let (send, recv) = tokio::try_join!(s_handle, r_handle)?;
    send.and(recv)?;
    Ok(())
}
