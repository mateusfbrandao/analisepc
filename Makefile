APP_NAME := analisepc
APP_ID := io.github.analisepc.AnalisePC
VERSION := 0.1.0
ARCH := amd64

PREFIX ?= /usr
BINDIR := $(DESTDIR)$(PREFIX)/bin
DATADIR := $(DESTDIR)$(PREFIX)/share
APPDIR := $(DATADIR)/applications
METAINFODIR := $(DATADIR)/metainfo
ICONDIR := $(DATADIR)/icons/hicolor

DEB_STAGE := target/debian/$(APP_NAME)_$(VERSION)_$(ARCH)
DEB_OUT := dist/$(APP_NAME)_$(VERSION)_$(ARCH).deb

.PHONY: all build run cli install install-user uninstall deb clean

all: build

build:
	cargo build --release

run:
	cargo run --release

cli:
	cargo run --release -- --cli

install: build
	install -Dm755 target/release/$(APP_NAME) "$(BINDIR)/$(APP_NAME)"
	install -Dm644 assets/$(APP_ID).desktop "$(APPDIR)/$(APP_ID).desktop"
	install -Dm644 assets/$(APP_ID).metainfo.xml "$(METAINFODIR)/$(APP_ID).metainfo.xml"
	install -Dm644 assets/icons/hicolor/scalable/apps/$(APP_ID).svg "$(ICONDIR)/scalable/apps/$(APP_ID).svg"
	install -Dm644 assets/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg "$(ICONDIR)/symbolic/apps/$(APP_ID)-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/analisepc-cpu-symbolic.svg "$(ICONDIR)/scalable/actions/analisepc-cpu-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/analisepc-ram-symbolic.svg "$(ICONDIR)/scalable/actions/analisepc-ram-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/analisepc-temp-symbolic.svg "$(ICONDIR)/scalable/actions/analisepc-temp-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/drive-harddisk-solidstate-symbolic.svg "$(ICONDIR)/scalable/actions/drive-harddisk-solidstate-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/application-x-firmware-symbolic.svg "$(ICONDIR)/scalable/actions/application-x-firmware-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/speedometer-symbolic.svg "$(ICONDIR)/scalable/actions/speedometer-symbolic.svg"
	-gtk-update-icon-cache -f -t "$(ICONDIR)" 2>/dev/null || true
	-update-desktop-database "$(APPDIR)" 2>/dev/null || true
	@echo "✅ Analise PC instalado com sucesso em $(BINDIR)/$(APP_NAME)"

install-user: build
	install -Dm755 target/release/$(APP_NAME) "$(HOME)/.local/bin/$(APP_NAME)"
	sed "s|^Exec=analisepc|Exec=$(HOME)/.local/bin/analisepc|" assets/$(APP_ID).desktop > "$(HOME)/.local/share/applications/$(APP_ID).desktop"
	install -Dm644 assets/icons/hicolor/scalable/apps/$(APP_ID).svg "$(HOME)/.local/share/icons/hicolor/scalable/apps/$(APP_ID).svg"
	install -Dm644 assets/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg "$(HOME)/.local/share/icons/hicolor/symbolic/apps/$(APP_ID)-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/analisepc-cpu-symbolic.svg "$(HOME)/.local/share/icons/hicolor/scalable/actions/analisepc-cpu-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/analisepc-ram-symbolic.svg "$(HOME)/.local/share/icons/hicolor/scalable/actions/analisepc-ram-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/analisepc-temp-symbolic.svg "$(HOME)/.local/share/icons/hicolor/scalable/actions/analisepc-temp-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/drive-harddisk-solidstate-symbolic.svg "$(HOME)/.local/share/icons/hicolor/scalable/actions/drive-harddisk-solidstate-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/application-x-firmware-symbolic.svg "$(HOME)/.local/share/icons/hicolor/scalable/actions/application-x-firmware-symbolic.svg"
	install -Dm644 assets/icons/hicolor/scalable/actions/speedometer-symbolic.svg "$(HOME)/.local/share/icons/hicolor/scalable/actions/speedometer-symbolic.svg"
	-gtk-update-icon-cache -f -t "$(HOME)/.local/share/icons/hicolor" 2>/dev/null || true
	-update-desktop-database "$(HOME)/.local/share/applications" 2>/dev/null || true
	@echo "✅ Analise PC instalado para o usuário atual em $(HOME)/.local/bin/$(APP_NAME)"

uninstall:
	rm -f "$(BINDIR)/$(APP_NAME)"
	rm -f "$(APPDIR)/$(APP_ID).desktop"
	rm -f "$(METAINFODIR)/$(APP_ID).metainfo.xml"
	rm -f "$(ICONDIR)/scalable/apps/$(APP_ID).svg"
	rm -f "$(ICONDIR)/symbolic/apps/$(APP_ID)-symbolic.svg"
	rm -f "$(ICONDIR)/scalable/actions/analisepc-cpu-symbolic.svg"
	rm -f "$(ICONDIR)/scalable/actions/analisepc-ram-symbolic.svg"
	rm -f "$(ICONDIR)/scalable/actions/analisepc-temp-symbolic.svg"
	-gtk-update-icon-cache -f -t "$(ICONDIR)" 2>/dev/null || true
	@echo "🗑️  Analise PC removido do sistema."

deb: build
	rm -rf "$(DEB_STAGE)"
	mkdir -p "$(DEB_STAGE)/DEBIAN"
	mkdir -p dist
	$(MAKE) install DESTDIR="$(CURDIR)/$(DEB_STAGE)" PREFIX=/usr
	@echo "Package: $(APP_NAME)" > "$(DEB_STAGE)/DEBIAN/control"
	@echo "Version: $(VERSION)" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo "Section: utils" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo "Priority: optional" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo "Architecture: $(ARCH)" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo "Depends: libgtk-4-1, libadwaita-1-0, pciutils, usbutils, iputils-ping" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo "Maintainer: Equipe Analise PC <contato@analisepc.github.io>" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo "Description: Monitoramento de hardware, diagnostico SMART, gargalo e benchmarks para Linux" >> "$(DEB_STAGE)/DEBIAN/control"
	@echo " Suite nativa em Rust, GTK 4 e Libadwaita para inspecao de hardware," >> "$(DEB_STAGE)/DEBIAN/control"
	@echo " telemetria termica, auditoria SMART, detector de gargalo e otimizacoes." >> "$(DEB_STAGE)/DEBIAN/control"
	chmod 755 "$(DEB_STAGE)/DEBIAN"
	chmod 644 "$(DEB_STAGE)/DEBIAN/control"
	dpkg-deb --build --root-owner-group "$(DEB_STAGE)" "$(DEB_OUT)"
	@echo "📦 Pacote Debian gerado com sucesso em: $(DEB_OUT)"

clean:
	cargo clean
	rm -rf dist target/debian
