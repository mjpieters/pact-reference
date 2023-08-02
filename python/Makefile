VERSION=0.4.7
OS=$(shell uname -s)
ARCH=$(shell uname -m)
build_ffi:
	cd ../rust/pact_ffi && \
	cargo build && \
	rustup run nightly cbindgen  \
		--crate pact_ffi \
		--output include/pact.h

get_ffi:
	curl -L -o pact.h https://github.com/pact-foundation/pact-reference/releases/download/libpact_ffi-v${VERSION}/pact.h && \
	case ${OS} in \
		Darwin) \
			case ${ARCH} in \
				"aarch64"|"arm64") \
					curl -L -o libpact_ffi.dylib.gz https://github.com/pact-foundation/pact-reference/releases/download/libpact_ffi-v${VERSION}/libpact_ffi-osx-aarch64-apple-darwin.dylib.gz && \
					gunzip -N -f libpact_ffi.dylib.gz;; \
				*) \
					curl -L -o libpact_ffi.dylib.gz https://github.com/pact-foundation/pact-reference/releases/download/libpact_ffi-v${VERSION}/libpact_ffi-osx-x86_64.dylib.gz && \
					gunzip -N -f libpact_ffi.dylib.gz;; \
			esac;;\
		Linux) \
			case ${ARCH} in \
				"aarch64"|"arm64") \
					curl -L -o libpact_ffi.so.gz https://github.com/pact-foundation/pact-reference/releases/download/libpact_ffi-v${VERSION}/libpact_ffi-linux-aarch64.so.gz && \
					gunzip -N -f libpact_ffi.so.gz;; \
				*) \
					curl -L -o libpact_ffi.so.gz https://github.com/pact-foundation/pact-reference/releases/download/libpact_ffi-v${VERSION}/libpact_ffi-linux-x86_64.so.gz && \
					gunzip -N -f libpact_ffi.so.gz;; \
			esac;;\
		CYGWIN*|MINGW32*|MSYS*|MINGW*) \
			case ${ARCH} in \
				*) \
					curl -L -o libpact_ffi.dll.gz https://github.com/pact-foundation/pact-reference/releases/download/libpact_ffi-v${VERSION}/libpact_ffi-windows-x86_64.dll.gz && \
					gunzip -N -f libpact_ffi.dll.gz;; \
			esac;;\
		*) \
			echo "ERROR: ${OS} is not a supported operating system" && \
			exit 1;;\
	esac

deps:
	pip install -r requires.txt

run_hello_ffi:
	python hello_ffi.py

run_pact_http:
	python pact_http_create_mock_server.py

run_pact_http_create_mock_server_for_pact:
	python pact_http_create_mock_server_for_pact.py

run_pact_message_v3:
	python pact_message_v3.py

run_pact_plugin_grpc_v4:
	python pact_plugin_grpc_v4.py

run_xml_consumer:
	pytest pact_http_xml_consumer.py -rP 	

test: deps run_pact_http run_pact_http_create_mock_server_for_pact run_pact_message_v3 run_pact_plugin_grpc_v4

pacts_show:
	cat "pacts/Consumer-Alice Service.json" | jq .
	cat "pacts/http-consumer-1-http-provider.json" | jq .
	cat "pacts/http-consumer-2-http-provider.json" | jq .
	cat "pacts/message-consumer-2-message-provider.json" | jq .
	cat "pacts/grpc-consumer-python-area-calculator-provider.json" | jq .