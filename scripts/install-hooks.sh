#!/usr/bin/env sh
set -e

cat > .git/hooks/pre-commit <<'EOF'
#!/usr/bin/env sh
set -e
cargo fmt -- --check 2>/dev/null || {
  echo "Running cargo fmt..."
  cargo fmt
  git add -u
}
EOF

chmod +x .git/hooks/pre-commit
echo "pre-commit hook installed"
