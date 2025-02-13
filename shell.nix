with import <nixpkgs> {};

pkgs.mkShell {
  buildInputs = with pkgs; [
    nodePackages.eslint
    nodePackages.prettier
    nodePackages.vscode-langservers-extracted
    openssl
    pkg-config
    python3
    python3Packages.black
    ruff
    rust-analyzer
    rustup
    sqlite
    sqlitebrowser
    jq
    gdb
  ];
}

