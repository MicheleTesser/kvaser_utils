pkgname=kvaser-utils
pkgver=0.1.0
pkgrel=1
pkgdesc="Kvaser Memorator Pro 2 utilities and TUI"
arch=('x86_64')
url="https://example.invalid/raceup/kvaser_utils"
license=('MIT' 'Apache')
depends=('gcc-libs' 'glibc' 'iproute2' 'sudo')
makedepends=('cargo' 'clang' 'make' 'perl')
options=('!lto')
source=()

build() {
  cd "$startdir"
  bash scripts/build-kvaser-local.sh
  cargo build --release --locked -p kvaser-tui
}

package() {
  cd "$startdir"

  install -Dm755 "target/release/kvaser-tui" "$pkgdir/usr/bin/kvaser-tui"
  install -Dm755 "scripts/kvaser_mode" "$pkgdir/usr/bin/kvaser_mode"
  install -Dm755 "scripts/kvaser-driver-mode.sh" "$pkgdir/usr/lib/kvaser_utils/kvaser-driver-mode.sh"
  install -Dm755 "scripts/setup-socketcan-tunnel-ifaces.sh" "$pkgdir/usr/bin/kvaser_socketcan_ifaces"

  install -d "$pkgdir/usr/lib/kvaser_utils"
  cp -d third_party/kvaser/linuxcan/canlib/libcanlib.so* "$pkgdir/usr/lib/kvaser_utils/"
  cp -d third_party/kvaser/kvlibsdk/kvadblib/so.ndb/libkvadblib.so* "$pkgdir/usr/lib/kvaser_utils/"
  cp -d third_party/kvaser/kvlibsdk/kvamemolibxml/so.ndb/libkvamemolibxml.so* "$pkgdir/usr/lib/kvaser_utils/"
  cp -d third_party/kvaser/kvlibsdk/kvmlib/kvmlib/so.ndb/libkvmlib.so* "$pkgdir/usr/lib/kvaser_utils/"
  cp -d third_party/kvaser/kvlibsdk/kvmlib/memolib_wrap/so.ndb/libkvamemolib.so* "$pkgdir/usr/lib/kvaser_utils/"
  cp -d third_party/kvaser/kvlibsdk/kvmlib/memolib_fd/so.ndb/libkvamemolib0700.so* "$pkgdir/usr/lib/kvaser_utils/"
  cp -d third_party/kvaser/kvlibsdk/kvlclib/so.ndb/libkvlclib.so* "$pkgdir/usr/lib/kvaser_utils/"

  install -Dm644 "README.md" "$pkgdir/usr/share/doc/$pkgname/README.md"
}
