# CARE CHILDREN








### 📦 Folder Structure

```text
care-children
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
└── compile.exe          # Script for UPX compression
