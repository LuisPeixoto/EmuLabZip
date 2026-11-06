import 'dart:typed_data';

import 'package:flutter/material.dart';

void main() => runApp(const DemoApp());

/// A list, a button that makes frames slow on purpose, and a button that allocates memory: enough to try the widget
/// tree, the frame timings and the memory view of EmuLabZip.
class DemoApp extends StatelessWidget {
  const DemoApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'EmuLabZip Flutter demo',
      theme: ThemeData(colorSchemeSeed: Colors.indigo, useMaterial3: true),
      home: const HomePage(),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key});

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  int _taps = 0;
  bool _slow = false;
  final List<Uint8List> _kept = [];

  void _allocate() {
    setState(() => _kept.add(Uint8List(2 * 1024 * 1024)));
  }

  @override
  Widget build(BuildContext context) {
    if (_slow) {
      // Blocks the UI thread for 50 ms on every build: about three frames' worth.
      final end = DateTime.now().add(const Duration(milliseconds: 50));
      while (DateTime.now().isBefore(end)) {}
    }
    return Scaffold(
      appBar: AppBar(title: const Text('Flutter demo')),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Wrap(
              spacing: 12,
              children: [
                FilledButton(key: const Key('count'), onPressed: () => setState(() => _taps++), child: Text('Tapped $_taps times')),
                OutlinedButton(key: const Key('jank'), onPressed: () => setState(() => _slow = !_slow), child: Text(_slow ? 'Jank: on' : 'Jank: off')),
                OutlinedButton(key: const Key('allocate'), onPressed: _allocate, child: Text('Allocate 2 MB (${_kept.length})')),
              ],
            ),
          ),
          Expanded(
            child: ListView.builder(
              itemCount: 40,
              itemBuilder: (context, i) => ListTile(leading: const Icon(Icons.eco), title: Text('Item $i')),
            ),
          ),
        ],
      ),
    );
  }
}
