// Uses some keys; `neverUsed` is not referenced anywhere.
Widget build(BuildContext context) {
  final t = AppLocalizations.of(context)!;
  return Text('${t.title} ${t.welcome("a", 1)} ${t.cartItems(2)} ${t.broken} ${t.noOther(1)}');
}
