import os
import subprocess


def _get_path(filename):
    return os.path.join(os.path.dirname(os.path.realpath(__file__)), '..', filename)


p = subprocess.Popen(['protoc',
                      _get_path("src/messages.proto"),
                      f'--proto_path={_get_path("src")}',
                      f'--python_out={_get_path("tests")}',
                      f'--pyi_out={_get_path("tests")}'])
p.wait()
os.system("sync")
print(p.stdout)
print(p.stderr)
