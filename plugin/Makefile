.PHONY: linux macos windows test clean

linux:
	../scripts/build_linux.sh

macos:
	../scripts/build_macos.sh

windows:
	powershell -ExecutionPolicy Bypass -File ../scripts/build_windows.ps1

test:
	../tests/mock_spi/run_mock_bridge_test.sh

clean:
	rm -rf ../target ../dist/*.plugin ../tests/mock_spi/build
