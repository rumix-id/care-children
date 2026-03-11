# CARE CHILDREN










care-children
├── .git/            # Git tidy folder (automatically exists after git init)
├── .gitignore       # File to exclude the target/db folder from GitHub
├── Cargo.toml       # Project & library configuration (dependencies)
├── Cargo.lock       # Library version lock
├── build.rs         # Script to insert manifest/icon during build
├── resource.rc      # Resource script (Icon & Metadata)
├── app.manifest     # Windows Administrator Permissions
├── cleanup_program.bat   # Script to remove regedit & startup traces
├── assets/               # Static assets folder
│ └── icon-img            # Icon and image
├── src/                  # Main source code folder
│ └── main.rs             # Iced & Proxy application logic
└── target/               # Build results folder (Will be created automatically by Rust)
└── release/
├── care-children.exe     # Compiled application files
├── upx                   # Place the UPX folder here
└── compress.bat          # Script for UPX compression
