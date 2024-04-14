vim.g.project_lspconfig = {
    rust_analyzer = {
        settings = {
            ["rust-analyzer"] = {
                check = {
                    allTargets = false,
                },
            },
        },
    },
}
