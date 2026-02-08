#!/bin/bash
# tor-setup.sh - Tor and torsocks installation script for Anarchy nodes
# Supports: Linux (apt/dnf), macOS (brew)

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command -v apt-get &> /dev/null; then
            echo "debian"
        elif command -v dnf &> /dev/null; then
            echo "fedora"
        elif command -v pacman &> /dev/null; then
            echo "arch"
        else
            echo "unknown-linux"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

# Install Tor and torsocks
install_tor() {
    local os=$(detect_os)
    
    case $os in
        debian)
            log_info "Installing Tor and torsocks on Debian/Ubuntu..."
            sudo apt-get update
            sudo apt-get install -y tor torsocks
            ;;
        fedora)
            log_info "Installing Tor and torsocks on Fedora..."
            sudo dnf install -y tor torsocks
            ;;
        arch)
            log_info "Installing Tor and torsocks on Arch..."
            sudo pacman -S --noconfirm tor torsocks
            ;;
        macos)
            log_info "Installing Tor and torsocks on macOS..."
            if ! command -v brew &> /dev/null; then
                log_error "Homebrew not found. Please install Homebrew first."
                exit 1
            fi
            brew install tor torsocks
            ;;
        *)
            log_error "Unsupported OS. Please install Tor and torsocks manually."
            exit 1
            ;;
    esac
}

# Start Tor service
start_tor() {
    local os=$(detect_os)
    
    case $os in
        debian|fedora|arch)
            log_info "Starting Tor service..."
            sudo systemctl enable tor
            sudo systemctl start tor
            ;;
        macos)
            log_info "Starting Tor service..."
            brew services start tor
            ;;
    esac
}

# Verify installation
verify_installation() {
    log_info "Verifying installation..."
    
    # Check Tor
    if command -v tor &> /dev/null; then
        local tor_version=$(tor --version | head -1)
        log_info "Tor installed: $tor_version"
    else
        log_error "Tor not found!"
        return 1
    fi
    
    # Check torsocks
    if command -v torsocks &> /dev/null; then
        local torsocks_version=$(torsocks --version 2>&1 | head -1)
        log_info "torsocks installed: $torsocks_version"
        
        # Check version (2.3+ required)
        local version_num=$(echo "$torsocks_version" | grep -oP '\d+\.\d+' | head -1)
        if [[ $(echo "$version_num >= 2.3" | bc -l 2>/dev/null || echo "1") == "1" ]]; then
            log_info "torsocks version OK (>= 2.3)"
        else
            log_warn "torsocks version may be too old. 2.3+ recommended."
        fi
    else
        log_error "torsocks not found!"
        return 1
    fi
    
    # Check Tor is running
    if pgrep -x "tor" > /dev/null; then
        log_info "Tor daemon is running"
    else
        log_warn "Tor daemon is not running. Starting..."
        start_tor
    fi
    
    return 0
}

# Test Tor connectivity
test_connectivity() {
    log_info "Testing Tor connectivity..."
    
    # Test via torsocks
    local tor_ip=$(torsocks curl -s https://check.torproject.org/api/ip 2>/dev/null | grep -oP '"IP":"[^"]+' | cut -d'"' -f4)
    
    if [[ -n "$tor_ip" ]]; then
        log_info "Tor connectivity OK! Exit node IP: $tor_ip"
        return 0
    else
        log_error "Failed to connect through Tor"
        return 1
    fi
}

# Main
main() {
    echo "========================================"
    echo "  Anarchy Node - Tor Setup Script"
    echo "========================================"
    echo ""
    
    local os=$(detect_os)
    log_info "Detected OS: $os"
    
    case "$1" in
        install)
            install_tor
            start_tor
            verify_installation
            ;;
        verify)
            verify_installation
            ;;
        test)
            test_connectivity
            ;;
        help|-h|--help)
            echo "Usage: $0 {install|verify|test|help}"
            echo ""
            echo "Commands:"
            echo "  install  - Install Tor and torsocks"
            echo "  verify   - Verify installation"
            echo "  test     - Test Tor connectivity"
            echo "  help     - Show this help message"
            exit 0
            ;;
        *)
            echo "Usage: $0 {install|verify|test|help}"
            echo ""
            echo "Commands:"
            echo "  install  - Install Tor and torsocks"
            echo "  verify   - Verify installation"
            echo "  test     - Test Tor connectivity"
            echo "  help     - Show this help message"
            exit 1
            ;;
    esac
}

main "$@"
