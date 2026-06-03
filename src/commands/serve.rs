//! `rqb serve` and `rqb mock` commands.

use std::path::Path;

use anyhow::Result;

use crate::{MockArgs, ServeArgs};

pub(crate) async fn serve(args: ServeArgs, collection: &Path) -> Result<()> {
    if args.host == "0.0.0.0" {
        eprintln!("Warning: binding to 0.0.0.0 exposes the local preview on your network.");
    }
    #[cfg(feature = "web")]
    {
        let root = args
            .path
            .unwrap_or_else(|| collection.parent().unwrap_or(Path::new(".")).to_path_buf());
        return reqbook::preview::run(root, &args.host, args.port, &args.env, args.mock).await;
    }
    #[cfg(not(feature = "web"))]
    {
        let _ = (args, collection);
        bail!(
            "web preview is not compiled into this binary\nFix: install Reqbook with default features."
        )
    }
}

pub(crate) async fn mock(args: MockArgs) -> Result<()> {
    #[cfg(feature = "web")]
    {
        reqbook::mock::run_mock_server(args.dir, args.port, args.latency).await
    }
    #[cfg(not(feature = "web"))]
    {
        let _ = args;
        bail!(
            "mock server is not compiled into this binary\nFix: install Reqbook with default features."
        )
    }
}
