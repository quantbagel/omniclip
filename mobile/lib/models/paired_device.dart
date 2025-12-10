import 'dart:convert';
import 'dart:typed_data';

class PairedDevice {
  final String id;
  final String name;
  final String host;
  final int port;
  final Uint8List publicKey;
  final Uint8List sessionKeyBytes;
  final DateTime pairedAt;

  PairedDevice({
    required this.id,
    required this.name,
    required this.host,
    required this.port,
    required this.publicKey,
    required this.sessionKeyBytes,
    DateTime? pairedAt,
  }) : pairedAt = pairedAt ?? DateTime.now();

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'host': host,
        'port': port,
        'publicKey': base64Encode(publicKey),
        'sessionKeyBytes': base64Encode(sessionKeyBytes),
        'pairedAt': pairedAt.toIso8601String(),
      };

  factory PairedDevice.fromJson(Map<String, dynamic> json) => PairedDevice(
        id: json['id'],
        name: json['name'],
        host: json['host'],
        port: json['port'],
        publicKey: base64Decode(json['publicKey']),
        sessionKeyBytes: base64Decode(json['sessionKeyBytes']),
        pairedAt: DateTime.parse(json['pairedAt']),
      );
}
