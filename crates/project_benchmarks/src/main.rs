use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use askpass::EncryptedPassword;
use clap::Parser;
use futures::channel::oneshot;
use gpui::AppContext as _;
use gpui::TaskExt;
use http_client::FakeHttpClient;
use language::LanguageRegistry;
use node_runtime::NodeRuntime;
use project::{
    Project, RealFs,
    search::{SearchQuery, SearchResult},
};
use release_channel::ReleaseChannel;
use semver::Version;

#[derive(Parser)]
struct Args {
    /// List of worktrees to run the search against.
    worktrees: Vec<String>,
    #[clap(short)]
    query: Option<String>,
    /// Askpass socket for SSH authentication
    #[clap(long)]
    askpass: Option<String>,
    /// Treat query as a regex.
    #[clap(short, long)]
    regex: bool,
    /// Matches have to be standalone words.
    #[clap(long)]
    whole_word: bool,
    /// Make matching case-sensitive.
    #[clap(long, default_value_t = false)]
    case_sensitive: bool,
    /// Include gitignored files in the search.
    #[clap(long)]
    include_ignored: bool,
}

fn main() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    if let Some(socket) = &args.askpass {
        askpass::main(socket);
        return Ok(());
    }

    let query_str = args
        .query
        .ok_or_else(|| anyhow!("-q/--query is required"))?;
    let query = if args.regex {
        SearchQuery::regex(
            query_str,
            args.whole_word,
            args.case_sensitive,
            args.include_ignored,
            false,
            Default::default(),
            Default::default(),
            false,
            None,
        )
    } else {
        SearchQuery::text(
            query_str,
            args.whole_word,
            args.case_sensitive,
            args.include_ignored,
            Default::default(),
            Default::default(),
            false,
            None,
        )
    }?;
    gpui_platform::headless().run(|cx| {
        release_channel::init_test(semver::Version::new(0, 0, 0), ReleaseChannel::Dev, cx);
        settings::init(cx);
        let client = client::new_http_client(cx);
        let http_client = FakeHttpClient::with_200_response();
        let (_, rx) = watch::channel(None);
        let node = NodeRuntime::new(http_client, None, rx);

        let registry = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
        let fs = RealFs::new(None, cx.background_executor().clone());



            cx.spawn(async move |cx| {
                let project = {
                    println!("Setting up local project");
                    cx.update(|cx| Project::local(
                    client,
                    node,

                    registry,
                    fs,
                    Some(Default::default()),
                    project::LocalProjectFlags {
                        init_worktree_trust: false,
                        ..Default::default()
                    },
                    cx,
                ))
                };
                println!("Loading worktrees");
                let worktrees = project.update(cx, |this, cx| {
                    args.worktrees
                        .into_iter()
                        .map(|worktree| this.find_or_create_worktree(worktree, true, cx))
                        .collect::<Vec<_>>()
                });

                let worktrees = futures::future::join_all(worktrees)
                    .await
                    .into_iter()
                    .collect::<Result<Vec<_>, anyhow::Error>>()?;

                for (worktree, _) in &worktrees {
                    let scan_complete = worktree
                        .update(cx, |this, _| {
                            if let Some(local) = this.as_local() {
                                Some(local.scan_complete())
                            } else {
                                None
                            }
                        });
                    if let Some(scan_complete) = scan_complete {
                        scan_complete.await;
                    } else {
                        cx.background_executor().timer(Duration::from_secs(10)).await;
                    }

                }
                println!("Worktrees loaded");

                println!("Starting a project search");
                let timer = std::time::Instant::now();
                let mut first_match = None;
                let matches = project.update(cx, |this, cx| this.search(query, cx));
                let mut matched_files = 0;
                let mut matched_chunks = 0;
                while let Ok(match_result) = matches.rx.recv().await {
                    if first_match.is_none() {
                        let time = timer.elapsed();
                        first_match = Some(time);
                        println!("First match found after {time:?}");
                    }
                    match match_result {
                        SearchResult::Buffer { ranges, .. } => {
                            matched_files += 1;
                            matched_chunks += ranges.len();
                        }
                        SearchResult::LimitReached => break,
                        SearchResult::WaitingForScan | SearchResult::Searching => continue,
                    }
                }
                let elapsed = timer.elapsed();
                println!(
                    "Finished project search after {elapsed:?}. Matched {matched_files} files and {matched_chunks} excerpts"
                );
                drop(project);
                cx.update(|cx| cx.quit());

                anyhow::Ok(())
            })
            .detach_and_log_err(cx);

    });
    Ok(())
}
