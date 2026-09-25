# https://github.com/nushell/nufmt/issues/134

def get-field [item key] { $item | get $key }
def f [is_new item] {
    let state = if $is_new {
      {}
      | upsert APP_DRIVER  (get-field $item "APP_DRIVER")
      | upsert APP_STAGE   (get-field $item "APP_STAGE")
    }
}
