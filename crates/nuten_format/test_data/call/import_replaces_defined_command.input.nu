module imported { export def replaced [x: string] {} }
def replaced [x: list] {}
use imported replaced
replaced [1,  2]
