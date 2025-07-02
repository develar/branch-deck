# Development Guide

## Prerequisites

- **Rust** (latest stable version)
- **Node.js** (18+ recommended) 
- **pnpm** package manager
- **Git**

## Setup

1. **Clone the repository**
   ```bash
   git clone <repository-url>
   cd branch-deck
   ```

2. **Install dependencies**
   ```bash
   pnpm install
   ```

3. **Install Rust dependencies** (handled automatically by Tauri)
   ```bash
   # Tauri will handle Rust dependencies during build
   ```

## Development Commands

### Running the Application

```bash
# Start development server with hot reload
pnpm tauri dev

# Alternative using npm scripts
pnpm dev
```

This will:
- Start the Vite dev server for the frontend
- Compile the Rust backend
- Launch the Tauri application window
- Enable hot reload for both frontend and backend changes

### Building

```bash
# Build for production
pnpm tauri build

# Build frontend only
pnpm build
```

### Testing

```bash
# Run all tests
pnpm test

# Run Rust tests only
cd src-tauri && cargo test --lib

# Run Rust tests with output
cd src-tauri && cargo test --lib -- --nocapture
```

### Linting and Code Quality

```bash
# Lint all code (Rust + TypeScript)
pnpm lint-all

# Lint Rust code only
pnpm lint-rust
# or
cd src-tauri && cargo clippy --fix -- -W clippy::all

# Lint TypeScript/Vue code only
pnpm lint
```

### Code Formatting

```bash
# Format Rust code
cd src-tauri && cargo fmt

# Format frontend code (handled by ESLint)
pnpm lint
```

## Project Structure

```
branch-deck/
├── src/                    # Vue.js frontend source
├── src-tauri/             # Rust backend source
│   ├── src/               # Rust source code
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
├── package.json           # Node.js dependencies and scripts
└── vite.config.ts         # Vite configuration
```

## Architecture

- **Frontend**: Vue.js 3 with Composition API, Nuxt UI components, TypeScript
- **Backend**: Rust with Tauri framework for native desktop app
- **Git Integration**: libgit2 Rust bindings for direct Git operations
- **IPC**: Tauri's built-in IPC for frontend-backend communication
- **Type Safety**: Specta for generating TypeScript types from Rust structs

## Key Technologies

- **Tauri**: Cross-platform desktop app framework
- **Vue 3**: Progressive JavaScript framework
- **Rust**: Systems programming language for performance
- **libgit2**: Native Git implementation
- **Vite**: Fast build tool and development server

## Development Workflow

1. Make changes to frontend code in `src/` or backend code in `src-tauri/src/`
2. The development server will automatically reload changes
3. Test your changes with `pnpm test`
4. Lint your code with `pnpm lint-all`
5. Build for production with `pnpm tauri build`

## Debugging

### Frontend Debugging
- Use browser dev tools (available in Tauri dev mode)
- Vue DevTools extension works in development

### Backend Debugging
- Use `println!` or `log::info!` for debugging
- Check console output in the terminal running `pnpm tauri dev`
- Use `cargo test -- --nocapture` to see test output

### Tauri-specific Debugging
- Enable Tauri DevTools in development mode
- Check Tauri logs in the console
- Use Tauri API documentation for IPC debugging

## Common Issues

1. **Build failures**: Ensure all dependencies are installed and Rust toolchain is up to date
2. **IPC errors**: Check that frontend and backend type definitions match
3. **Git operations failing**: Ensure libgit2 is properly linked (handled by cargo features)

## Contributing

1. Follow the existing code style (enforced by linters)
2. Add tests for new functionality
3. Update documentation as needed
4. Ensure all tests pass before submitting changes
