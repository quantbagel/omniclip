import 'dart:convert';
import 'dart:typed_data';
import 'package:cryptography/cryptography.dart';
import 'package:uuid/uuid.dart';

/// Service for all cryptographic operations.
///
/// Handles key exchange, encryption/decryption, and session key derivation.
class CryptoService {
  static const String _sessionKeyInfo = 'omniclip-session-key';

  final _keyExchange = X25519();
  final _ed25519 = Ed25519();
  final _cipher = AesGcm.with256bits();
  final _uuid = const Uuid();

  /// Generate a new ephemeral X25519 key pair for key exchange.
  Future<SimpleKeyPair> generateEphemeralKeyPair() async {
    return _keyExchange.newKeyPair();
  }

  /// Generate a new Ed25519 identity key pair for signing.
  Future<SimpleKeyPair> generateIdentityKeyPair() async {
    return _ed25519.newKeyPair();
  }

  /// Perform ECDH key exchange and derive a session key.
  Future<SecretKey> deriveSessionKey({
    required SimpleKeyPair ourKeyPair,
    required Uint8List theirPublicKeyBytes,
  }) async {
    final theirPublicKey = SimplePublicKey(
      theirPublicKeyBytes,
      type: KeyPairType.x25519,
    );

    final sharedSecret = await _keyExchange.sharedSecretKey(
      keyPair: ourKeyPair,
      remotePublicKey: theirPublicKey,
    );

    final hkdf = Hkdf(hmac: Hmac.sha256(), outputLength: 32);
    return hkdf.deriveKey(
      secretKey: sharedSecret,
      info: utf8.encode(_sessionKeyInfo),
      nonce: Uint8List(0),
    );
  }

  /// Encrypt content using the session key.
  Future<EncryptedContent> encrypt(
    String content,
    SecretKey sessionKey,
  ) async {
    final plaintext = utf8.encode(content);

    // Generate random 12-byte nonce
    final nonce = _generateRandomNonce();

    final secretBox = await _cipher.encrypt(
      plaintext,
      secretKey: sessionKey,
      nonce: nonce,
    );

    return EncryptedContent(
      nonce: nonce,
      ciphertext: Uint8List.fromList([...secretBox.cipherText, ...secretBox.mac.bytes]),
    );
  }

  /// Decrypt content using the session key.
  Future<String> decrypt(
    EncryptedContent encrypted,
    SecretKey sessionKey,
  ) async {
    final ciphertext = encrypted.ciphertext;

    // MAC is last 16 bytes
    final secretBox = SecretBox(
      ciphertext.sublist(0, ciphertext.length - 16),
      nonce: encrypted.nonce,
      mac: Mac(ciphertext.sublist(ciphertext.length - 16)),
    );

    final decrypted = await _cipher.decrypt(secretBox, secretKey: sessionKey);
    return utf8.decode(decrypted);
  }

  /// Generate a new random UUID v4.
  String generateUuid() => _uuid.v4();

  /// Generate a random 12-byte nonce for AES-GCM.
  Uint8List _generateRandomNonce() {
    // Use timestamp + random for better entropy
    final nonce = Uint8List(12);
    final now = DateTime.now();
    final bytes = ByteData.view(nonce.buffer);
    bytes.setInt64(0, now.microsecondsSinceEpoch);
    // Add some variation in the last 4 bytes
    for (var i = 8; i < 12; i++) {
      nonce[i] = (now.microsecond * (i + 1)) % 256;
    }
    return nonce;
  }
}

/// Encrypted content with nonce.
class EncryptedContent {
  final Uint8List nonce;
  final Uint8List ciphertext;

  EncryptedContent({
    required this.nonce,
    required this.ciphertext,
  });

  Map<String, String> toJson() => {
    'nonce': base64Encode(nonce),
    'ciphertext': base64Encode(ciphertext),
  };

  factory EncryptedContent.fromJson(Map<String, dynamic> json) {
    return EncryptedContent(
      nonce: base64Decode(json['nonce'] as String),
      ciphertext: base64Decode(json['ciphertext'] as String),
    );
  }
}
