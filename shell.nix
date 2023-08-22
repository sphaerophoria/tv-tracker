with import <nixpkgs> {};

pkgs.mkShell {
  buildInputs = with pkgs; [
    nodePackages_latest.eslint
    nodePackages_latest.prettier
    nodePackages_latest.vscode-langservers-extracted
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

