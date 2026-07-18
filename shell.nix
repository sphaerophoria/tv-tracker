with import <nixpkgs> {};

pkgs.mkShell {
  buildInputs = with pkgs; [
    eslint
    prettier
    vscode-langservers-extracted
    openssl
    pkg-config
    python3
    python3Packages.black
    ruff
    sqlite
    sqlitebrowser
    jq
    gdb
    zls
    zig
  ];
}

