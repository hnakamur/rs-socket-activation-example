INSTALL_FILES = \
	/etc/systemd/system/sockact.socket \
	/etc/systemd/system/sockact.service \
	/var/lib/sockact/bin/sockact

install: $(INSTALL_FILES)
	sudo systemctl restart sockact

/etc/systemd/system/sockact.socket: sockact.socket
	sudo install sockact.socket /etc/systemd/system/sockact.socket
	sudo systemctl daemon-reload

/etc/systemd/system/sockact.service: sockact.service
	sudo install sockact.service /etc/systemd/system/sockact.service
	sudo systemctl daemon-reload

target/release/sockact: src/main.rs
	if ! dpkg -s libsystemd-dev >/dev/null 2>&1; then sudo apt-get install -y libsystemd-dev; fi
	cargo build --release

/var/lib/sockact/bin/sockact: target/release/sockact
	sudo mkdir -p /var/lib/sockact/bin
	sudo install ./target/release/sockact /var/lib/sockact/bin/sockact

clean:
	sudo rm -f $(INSTALL_FILES)
	sudo systemctl daemon-reload

.PHONY: install clean
