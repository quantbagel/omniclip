import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';
import 'package:flutter/foundation.dart';

/// Service for message encoding, decoding, and framing.
///
/// Handles the length-prefixed protocol for TCP transport.
class MessageService {
  static const int maxMessageSize = 10 * 1024 * 1024; // 10 MB

  /// Encode a message map to bytes with length prefix.
  Uint8List encodeMessage(Map<String, dynamic> message) {
    final jsonBytes = utf8.encode(jsonEncode(message));
    final lengthBytes = ByteData(4)..setUint32(0, jsonBytes.length, Endian.big);

    return Uint8List.fromList([
      ...lengthBytes.buffer.asUint8List(),
      ...jsonBytes,
    ]);
  }

  /// Send a message through a socket with length prefix.
  Future<void> sendMessage(Socket socket, Map<String, dynamic> message) async {
    final msgBytes = utf8.encode(jsonEncode(message));
    final lengthBytes = ByteData(4)..setUint32(0, msgBytes.length, Endian.big);

    socket.add(lengthBytes.buffer.asUint8List());
    socket.add(msgBytes);
    await socket.flush();

    debugPrint('Sent message (${msgBytes.length} bytes)');
  }

  /// Decode a JSON message from bytes.
  Map<String, dynamic>? decodeMessage(List<int> payload) {
    try {
      final jsonStr = utf8.decode(payload, allowMalformed: true);
      if (jsonStr.startsWith('{')) {
        return jsonDecode(jsonStr) as Map<String, dynamic>;
      }
    } catch (e) {
      debugPrint('Failed to decode message: $e');
    }
    return null;
  }

  /// Build a PairRequest message in Rust serde format.
  Map<String, dynamic> buildPairRequest({
    required String sessionId,
    required String deviceId,
    required String deviceName,
    required String ephemeralPubkeyB64,
    required String identityPubkeyB64,
  }) {
    return {
      'PairRequest': {
        'session_id': sessionId,
        'device_id': deviceId,
        'device_name': deviceName,
        'ephemeral_pubkey': ephemeralPubkeyB64,
        'identity_pubkey': identityPubkeyB64,
      },
    };
  }

  /// Build a ClipboardSync message.
  Map<String, dynamic> buildClipboardSync({
    required String messageId,
    required String senderId,
    required String contentHashB64,
    required Map<String, String> encryptedContent,
    required int timestamp,
  }) {
    return {
      'ClipboardSync': {
        'message_id': messageId,
        'sender_id': senderId,
        'content_hash': contentHashB64,
        'encrypted_content': encryptedContent,
        'timestamp': timestamp,
      },
    };
  }
}

/// Parser for length-prefixed message frames from a stream.
class FrameParser {
  final BytesBuilder _buffer = BytesBuilder();
  int? _expectedLength;

  /// Add incoming bytes to the buffer.
  void addBytes(List<int> bytes) {
    _buffer.add(bytes);
  }

  /// Extract any complete frames from the buffer.
  /// Returns a list of complete message payloads.
  List<Uint8List> extractFrames() {
    final frames = <Uint8List>[];

    while (true) {
      // Try to read length prefix
      if (_expectedLength == null && _buffer.length >= 4) {
        final allBytes = _buffer.takeBytes();
        _expectedLength = ByteData.view(
          Uint8List.fromList(allBytes.sublist(0, 4)).buffer,
        ).getUint32(0, Endian.big);
        _buffer.add(allBytes.sublist(4));

        // Validate message size
        if (_expectedLength! > MessageService.maxMessageSize) {
          debugPrint('Message too large: $_expectedLength bytes');
          _expectedLength = null;
          return frames;
        }
      }

      // Try to read complete message
      if (_expectedLength != null && _buffer.length >= _expectedLength!) {
        final allBytes = _buffer.takeBytes();
        final payload = Uint8List.fromList(allBytes.sublist(0, _expectedLength!));
        _buffer.add(allBytes.sublist(_expectedLength!));
        _expectedLength = null;
        frames.add(payload);
      } else {
        break;
      }
    }

    return frames;
  }

  /// Clear the buffer.
  void clear() {
    _buffer.clear();
    _expectedLength = null;
  }
}
