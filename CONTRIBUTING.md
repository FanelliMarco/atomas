# Contributing to Atomas

Thank you for your interest in contributing to Atomas! This document provides guidelines and instructions for contributing.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Testing Guidelines](#testing-guidelines)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)
- [Code Style](#code-style)

## 🤝 Code of Conduct

- Be respectful and inclusive
- Provide constructive feedback
- Focus on what's best for the project
- Show empathy towards other contributors

## 🚀 Getting Started

### Prerequisites

- Rust 1.70+ (`rustup update`)
- OpenCV 4.x development libraries
- Git
- Docker (for emulator development)

### Fork and Clone

```bash
# Fork the repository on GitHub, then:
git clone https://github.com/YOUR_USERNAME/atomas.git
cd atomas
git remote add upstream https://github.com/ORIGINAL_OWNER/atomas.git
```

## 💻 Development Setup

### Local Build

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt-get install libopencv-dev clang libclang-dev

# Build the project
cargo build

# Run tests
cargo test --all

# Run with logging
RUST_LOG=debug cargo run
```

### Docker Development

```bash
# Build development container
docker build -f docker/Dockerfile -t atomas-dev .

# Run interactive development session
docker run -it --privileged \
  -v $(pwd):/atomas \
  -p 5037:5037 \
  atomas-dev bash
```

## 📁 Project Structure

```
atomas/
├── crates/
│   ├── atomas-core/          # Core game logic
│   │   ├── elements/         # Element definitions and data
│   │   └── ring/             # Ring structures and adjacency
│   └── atomas-cv/            # Computer vision library
│       ├── bbox.rs           # Bounding box utilities
│       ├── detection/        # Game state detection
│       ├── template/         # Template matching
│       └── utils/            # Image processing utilities
├── src/
│   ├── gamestate.rs          # Game state representation
│   ├── parser.rs             # CV-based parsing
│   └── main.rs               # Application entry point
├── assets/
│   ├── txt/elements.txt      # Element database
│   └── png/                  # Template images
└── docker/                   # Containerization
```

## 🔧 Making Changes

### Areas for Contribution

#### 1. Computer Vision Improvements
- New template matching algorithms
- Better preprocessing methods
- Multi-scale detection improvements
- Edge detection enhancements

#### 2. Core Game Logic
- Game mechanics simulation
- Move validation
- Scoring system
- Advanced graph algorithms

#### 3. Template Assets
- Higher quality element templates
- Multi-resolution templates
- Special atom variants

#### 4. Testing
- Unit tests for CV modules
- Integration tests
- Benchmark tests
- Test fixtures and data

#### 5. Documentation
- API documentation
- Tutorials and examples
- Algorithm explanations
- Performance optimization guides

### Branch Naming

```
feature/add-new-detection-method
bugfix/template-matching-accuracy
docs/api-documentation
refactor/detection-pipeline
```

## 🧪 Testing Guidelines

### Writing Tests

```rust
// crates/atomas-cv/tests/detection_tests.rs
#[test]
fn test_element_detection() -> Result<()> {
    let config = DetectionConfig::default();
    let detector = GameStateDetector::new(config)?;
    
    let result = detector.detect_from_file(
        "tests/fixtures/sample_board.jpg",
        &test_data()
    )?;
    
    assert!(result.ring_elements.len() >= 6);
    assert!(result.player_atom.is_some());
    Ok(())
}
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific crate tests
cargo test -p atomas-cv

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --all --out Html
```

## 📝 Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

### Examples

```
feat(cv): add Sobel gradient preprocessing method

Implement Sobel gradient magnitude calculation for template
preprocessing to improve matching under varying lighting conditions.

Closes #42
```

```
fix(detection): correct NMS threshold application

Fixed issue where NMS was not properly filtering overlapping
detections for the same element class.

Fixes #38
```

## 🔀 Pull Request Process

### Before Submitting

1. **Update from upstream**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run tests and linting**
   ```bash
   cargo test --all
   cargo clippy --all-targets --all-features
   cargo fmt --all -- --check
   ```

3. **Update documentation**
   ```bash
   cargo doc --no-deps --open
   ```

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] All tests pass
- [ ] Added new tests
- [ ] Manual testing performed

## Checklist
- [ ] Code follows project style
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No new warnings
```

### Review Process

1. Automated checks must pass
2. At least one maintainer approval required
3. No unresolved conversations
4. Up-to-date with main branch

## 🎨 Code Style

### Rust Style

Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

```rust
// Use meaningful names
let detection_result = detector.detect_from_file(path, data)?;

// Document public APIs
/// Detects game state from an image file
///
/// # Arguments
/// * `image_path` - Path to the screenshot
/// * `elements_data` - Element database
///
/// # Returns
/// Detected game state with confidence scores
pub fn detect_game_state(/* ... */) -> Result<GameState> {
    // ...
}

// Prefer explicit error handling
match detector.detect(image) {
    Ok(result) => process_result(result),
    Err(e) => return Err(e.into()),
}
```

### Formatting

```bash
# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Lint code
cargo clippy --all-targets --all-features -- -D warnings
```

## 📚 Additional Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [OpenCV Documentation](https://docs.opencv.org/)
- [Template Matching Guide](https://docs.opencv.org/4.x/d4/dc6/tutorial_py_template_matching.html)
- [Project Issues](https://github.com/OWNER/atomas/issues)

## 💬 Getting Help

- Open an [issue](https://github.com/OWNER/atomas/issues) for bugs
- Start a [discussion](https://github.com/OWNER/atomas/discussions) for questions
- Join our Discord (link in README)

## 🙏 Recognition

Contributors will be acknowledged in:
- README.md contributors section
- Release notes
- Project documentation

Thank you for contributing to Atomas! 🎉
