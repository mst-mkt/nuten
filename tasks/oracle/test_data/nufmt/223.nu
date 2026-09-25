# https://github.com/nushell/nufmt/issues/223

def --wrapped main [config: string --binary: string = "" ...rest: string] { $config }
def main2 [bin: string --config: string --duration: int = 120] { $bin }
