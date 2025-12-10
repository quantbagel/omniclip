import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:cryptography/cryptography.dart';
import 'package:shared_preferences/shared_preferences.dart';
import '../models/paired_device.dart';
import 'crypto_service.dart';
import 'message_service.dart';

/// Service for managing device pairing and clipboard synchronization.
///
/// Uses [CryptoService] for all cryptographic operations and
/// [MessageService] for protocol message handling.
class SyncService extends ChangeNotifier {
  final CryptoService _crypto = CryptoService();
  final MessageService _messages = MessageService();

  final List<PairedDevice> _pairedDevices = [];
  Socket? _activeSocket;
  bool _isConnected = false;
  String? _lastClipboard;
  StreamSubscription? _socketSubscription;

  // Session state
  SimpleKeyPair? _keyPair;
  SimpleKeyPair? _identityKeyPair;
  SecretKey? _sessionKey;
  PairedDevice? _connectedDevice;
  String? _deviceId;

  List<PairedDevice> get pairedDevices => List.unmodifiable(_pairedDevices);
  bool get isConnected => _isConnected;
  String? get connectedDeviceId => _connectedDevice?.id;
  String? get lastClipboard => _lastClipboard;

  SyncService() {
    _loadPairedDevices();
  }

  Future<void> _loadPairedDevices() async {
    final prefs = await SharedPreferences.getInstance();
    final json = prefs.getString('paired_devices');
    if (json != null) {
      final list = jsonDecode(json) as List;
      _pairedDevices.addAll(list.map((e) => PairedDevice.fromJson(e)));
      notifyListeners();
    }
  }

  Future<void> _savePairedDevices() async {
    final prefs = await SharedPreferences.getInstance();
    final json = jsonEncode(_pairedDevices.map((e) => e.toJson()).toList());
    await prefs.setString('paired_devices', json);
  }

  /// Parse pairing URL from QR code.
  PairingData? parsePairingUrl(String url) {
    try {
      url = url.replaceAll(RegExp(r'\s+'), '');
      debugPrint('Parsing URL: $url');
      if (!url.startsWith('omniclip://pair?')) {
        debugPrint('URL does not start with omniclip://pair?');
        return null;
      }

      final uri = Uri.parse(url.replaceFirst('omniclip://', 'https://'));
      debugPrint('Query params: ${uri.queryParameters}');

      final keyB64 = uri.queryParameters['k']!;
      final paddedKey = keyB64.padRight((keyB64.length + 3) ~/ 4 * 4, '=');
      debugPrint('Key (padded): $paddedKey');

      return PairingData(
        sessionId: uri.queryParameters['s']!,
        publicKey: base64Url.decode(paddedKey),
        host: uri.queryParameters['h']!,
        port: int.parse(uri.queryParameters['p']!),
        deviceName: uri.queryParameters['n'] ?? 'Unknown',
      );
    } catch (e, stack) {
      debugPrint('Failed to parse pairing URL: $e');
      debugPrint('Stack: $stack');
      return null;
    }
  }

  /// Connect to a device using pairing data from QR code.
  Future<bool> connectToPeer(PairingData pairingData) async {
    try {
      // Generate ephemeral key pair
      _keyPair = await _crypto.generateEphemeralKeyPair();
      final ourPublicKey = await _keyPair!.extractPublicKey();

      // Connect to the peer
      _activeSocket = await Socket.connect(
        pairingData.host,
        pairingData.port,
        timeout: const Duration(seconds: 10),
      );

      // Derive session key via ECDH
      _sessionKey = await _crypto.deriveSessionKey(
        ourKeyPair: _keyPair!,
        theirPublicKeyBytes: pairingData.publicKey,
      );

      // Create paired device record
      _connectedDevice = PairedDevice(
        id: pairingData.sessionId,
        name: pairingData.deviceName,
        host: pairingData.host,
        port: pairingData.port,
        publicKey: pairingData.publicKey,
        sessionKeyBytes: Uint8List.fromList(await _sessionKey!.extractBytes()),
      );

      // Send pairing response
      await _sendPairingResponse(pairingData.sessionId, ourPublicKey);

      _isConnected = true;
      notifyListeners();

      // Start listening for messages
      _listenForMessages();

      // Save paired device
      _pairedDevices.add(_connectedDevice!);
      await _savePairedDevices();

      return true;
    } catch (e) {
      debugPrint('Connection failed: $e');
      await disconnect();
      return false;
    }
  }

  Future<void> _sendPairingResponse(
      String sessionId, SimplePublicKey ourPublicKey) async {
    // Generate identity key pair if needed
    _identityKeyPair ??= await _crypto.generateIdentityKeyPair();
    final identityPublicKey = await _identityKeyPair!.extractPublicKey();

    // Generate device ID if not set
    _deviceId ??= _crypto.generateUuid();

    final message = _messages.buildPairRequest(
      sessionId: sessionId,
      deviceId: _deviceId!,
      deviceName: 'Omniclip Mobile',
      ephemeralPubkeyB64: base64Encode(ourPublicKey.bytes),
      identityPubkeyB64: base64Encode(identityPublicKey.bytes),
    );

    debugPrint('Sending pairing message: ${jsonEncode(message)}');
    await _messages.sendMessage(_activeSocket!, message);
    debugPrint('Pairing message sent');
  }

  void _listenForMessages() {
    final frameParser = FrameParser();

    _socketSubscription = _activeSocket!.listen(
      (data) async {
        frameParser.addBytes(data);

        for (final payload in frameParser.extractFrames()) {
          await _handleMessage(payload);
        }
      },
      onError: (e) {
        debugPrint('Socket error: $e');
        disconnect();
      },
      onDone: () {
        debugPrint('Socket closed');
        disconnect();
      },
    );
  }

  Future<void> _handleMessage(List<int> payload) async {
    try {
      final msg = _messages.decodeMessage(payload);
      if (msg == null) return;

      debugPrint('Received message: ${msg.keys.first}');

      // Handle different message types
      if (msg.containsKey('ClipboardSync')) {
        await _handleClipboardSync(msg['ClipboardSync']);
      } else if (msg['type'] == 'ClipboardSync') {
        // Legacy format
        await _handleClipboardSync(msg);
      }
    } catch (e) {
      debugPrint('Failed to handle message: $e');
    }
  }

  Future<void> _handleClipboardSync(Map<String, dynamic> msg) async {
    if (_sessionKey == null) return;

    try {
      final encryptedContent = msg['encrypted_content'];
      final encrypted = EncryptedContent.fromJson(encryptedContent);
      final content = await _crypto.decrypt(encrypted, _sessionKey!);

      debugPrint('Received clipboard: $content');
      _lastClipboard = content;
      notifyListeners();
    } catch (e) {
      debugPrint('Failed to decrypt clipboard: $e');
    }
  }

  /// Send clipboard content to connected peer.
  Future<void> sendClipboard(String content) async {
    if (!_isConnected || _activeSocket == null || _sessionKey == null) return;

    try {
      final encrypted = await _crypto.encrypt(content, _sessionKey!);

      final message = _messages.buildClipboardSync(
        messageId: _crypto.generateUuid(),
        senderId: _deviceId ?? 'unknown',
        contentHashB64: base64Encode(utf8.encode(content)),
        encryptedContent: encrypted.toJson(),
        timestamp: DateTime.now().millisecondsSinceEpoch ~/ 1000,
      );

      await _messages.sendMessage(_activeSocket!, message);
      debugPrint(
          'Sent clipboard: ${content.substring(0, content.length.clamp(0, 50))}...');
    } catch (e) {
      debugPrint('Failed to send clipboard: $e');
    }
  }

  Future<void> disconnect() async {
    _socketSubscription?.cancel();
    _socketSubscription = null;
    await _activeSocket?.close();
    _activeSocket = null;
    _isConnected = false;
    _sessionKey = null;
    _keyPair = null;
    _connectedDevice = null;
    notifyListeners();
  }

  Future<void> removePairedDevice(String id) async {
    _pairedDevices.removeWhere((d) => d.id == id);
    await _savePairedDevices();
    notifyListeners();
  }

  @override
  void dispose() {
    disconnect();
    super.dispose();
  }
}

/// Data parsed from a pairing QR code URL.
class PairingData {
  final String sessionId;
  final Uint8List publicKey;
  final String host;
  final int port;
  final String deviceName;

  PairingData({
    required this.sessionId,
    required this.publicKey,
    required this.host,
    required this.port,
    required this.deviceName,
  });
}
