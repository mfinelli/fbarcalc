PREFIX := /usr/local
DESTDIR :=

all: target/release/fbarcalc

clean:
	rm -rf target

target/release/fbarcalc: $(wildcard src/*.rs)
	cargo build --frozen --release

install:
	install -Dm0755 target/release/fbarcalc \
		"$(DESTDIR)$(PREFIX)/bin/fbarcalc"
	install -Dm0644 README.md \
		"$(DESTDIR)$(PREFIX)/share/doc/fbarcalc/README.md"
	install -Dm0644 fbarcalc.1 \
		"$(DESTDIR)$(PREFIX)/share/man/man1/fbarcalc.1"

uninstall:
	rm -rf \
		"$(DESTDIR)$(PREFIX)/bin/fbarcalc"
		"$(DESTDIR)$(PREFIX)/share/doc/fbarcalc"
		"$(DESTDIR)$(PREFIX)/share/man/man1/fbarcalc.1"

test:
	./target/release/fbarcalc -V

.PHONY: all clean install test uninstall
