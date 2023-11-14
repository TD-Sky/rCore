vim.g.project_lspconfig = {
    rust_analyzer = {
        settings = {
            ["rust-analyzer"] = {
                checkOnSave = {
                    allTargets = false,
                },
            },
        },
    },
}
