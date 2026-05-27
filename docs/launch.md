# Open-source launch plan

The goal is not a one-day spike. The goal is to get a small number of real developers to install Trellis, run it on an existing project, and report friction.

## Audience

Primary:

- Developers using AI coding agents
- Developers who prefer markdown and Git-native workflows
- Teams that dislike heavy API clients for simple API test flows

Secondary:

- Rust CLI users
- API platform engineers
- OSS maintainers who need runnable API docs

## Positioning

Use this message consistently:

> Trellis is a local-first, markdown-native API spec and workflow tool built for AI coding agents.

Avoid vague positioning like "better Postman". Instead, show the concrete workflow:

1. Scan or import endpoints.
2. Review markdown specs in Git.
3. Run from CLI or browser.
4. Build a flow canvas.
5. Save the flow back to markdown.
6. Let an agent author, run, or debug specs using skills.

## Launch assets

Prepare before posting:

- 30 second web preview GIF
- 30 second flow canvas GIF
- 30 second agent skill GIF
- `examples/jsonplaceholder` working from a fresh clone
- A short README quickstart with three commands
- A pinned GitHub issue asking for beta feedback

## Channels

Start small:

- GitHub release and discussion
- Project README
- Personal X/LinkedIn demo clip
- Rust community post
- Web/API developer community post
- Show HN only after install friction is low

## Feedback targets

Ask testers to try:

```bash
cargo install trellis
trellis init --name=demo --dev-url=https://jsonplaceholder.typicode.com --yes
trellis serve
```

Ask for:

- Install time and OS
- Whether the first page opened
- Whether endpoint execution worked
- Whether the flow canvas made sense
- Whether the markdown output was readable

## Metrics

Track:

- GitHub release downloads
- crates.io downloads
- npm downloads
- Docker pulls
- Stars and forks
- Issues opened by real users
- Successful external smoke tests

Do not add telemetry by default. If telemetry is ever introduced, it should be explicit opt-in.
