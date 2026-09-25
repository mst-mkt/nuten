# https://github.com/nushell/nufmt/issues/180

alias wds = systemd-run waydroid session start
alias wdo = waydroid session stop
def wdr [] {
    wdo
    wds
}
