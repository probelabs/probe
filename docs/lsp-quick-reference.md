# LSP Quick Reference

## Common Flow

```bash
probe lsp status
probe lsp index --workspace .
probe lsp index-status
probe extract src/main.rs#main --lsp
```

## Daemon Commands

```bash
probe lsp start -f
probe lsp restart
probe lsp shutdown
probe lsp ping
probe lsp languages
```

## Logs and Diagnostics

```bash
probe lsp logs -n 200
probe lsp logs --follow
probe lsp logs --analyze -n 50000 --top 50
probe lsp doctor
probe lsp crash-logs
```

## Direct LSP Calls

```bash
probe lsp call definition src/main.rs#main
probe lsp call references src/main.rs:42:10
probe lsp call hover src/main.rs#main
probe lsp call document-symbols src/main.rs
probe lsp call workspace-symbols main
probe lsp call call-hierarchy src/main.rs#main
probe lsp call implementations src/main.rs#SomeTrait
probe lsp call type-definition src/main.rs:42:10
probe lsp call fqn src/main.rs#main
```

## Indexing Commands

```bash
probe lsp index --workspace .
probe lsp index --workspace . --recursive
probe lsp index --workspace . --wait
probe lsp index-stop
probe lsp index-config --help
probe lsp index-export --help
```

## Useful Help Commands

```bash
probe lsp --help
probe lsp call --help
probe lsp index --help
probe lsp logs --help
```
