import 'package:flutter/material.dart';
import 'package:flutter_gen/gen_l10n/app_localizations.dart';

class Home extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Column(children: [
      Text(l10n.appTitle),
      Text(l10n.greeting('Amina')),
      Text(AppLocalizations.of(context)!
          .itemCount(3)),
    ]);
  }
}
