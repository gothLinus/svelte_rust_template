import type {
	CredentialDescriptor,
	PasskeyAssertionRequestSchema,
	PasskeyCreationOptions,
	PasskeyRequestOptions,
	RegisterPasskeyRequestSchema
} from '$lib/types/api';
import type { Init } from '$lib/api/client';

/**
 * Passkeys in the browser: turns the server's options into what `navigator.credentials`
 * takes, and its results back into request messages. Binary values travel as bytes both
 * ways, so nothing is encoded or decoded here.
 */

export function passkeysSupported(): boolean {
	return typeof window !== 'undefined' && typeof window.PublicKeyCredential === 'function';
}

export class PasskeyCancelled extends Error {
	constructor() {
		super('The passkey prompt was closed.');
		this.name = 'PasskeyCancelled';
	}
}

function cancelled(error: unknown): boolean {
	return error instanceof DOMException && ['NotAllowedError', 'AbortError'].includes(error.name);
}

function incomplete(): Error {
	return new Error('The server sent incomplete passkey options.');
}

function bytes(value: Uint8Array | ArrayBuffer): Uint8Array<ArrayBuffer> {
	return new Uint8Array(value);
}

function descriptor(descriptor: CredentialDescriptor): PublicKeyCredentialDescriptor {
	return {
		type: 'public-key',
		id: bytes(descriptor.id),
		transports: descriptor.transports as AuthenticatorTransport[]
	};
}

export async function createPasskey(
	options: PasskeyCreationOptions,
	name: string
): Promise<Init<typeof RegisterPasskeyRequestSchema>> {
	const { publicKey } = options;
	if (!publicKey?.rp || !publicKey.user || !publicKey.authenticatorSelection) throw incomplete();
	let credential: Credential | null;
	try {
		credential = await navigator.credentials.create({
			publicKey: {
				rp: { id: publicKey.rp.id, name: publicKey.rp.name },
				user: {
					id: bytes(publicKey.user.id),
					name: publicKey.user.name,
					displayName: publicKey.user.displayName
				},
				challenge: bytes(publicKey.challenge),
				pubKeyCredParams: publicKey.algorithms.map((alg) => ({ type: 'public-key', alg })),
				timeout: publicKey.timeout,
				excludeCredentials: publicKey.excludeCredentials.map(descriptor),
				authenticatorSelection: {
					residentKey: publicKey.authenticatorSelection.residentKey as ResidentKeyRequirement,
					userVerification: publicKey.authenticatorSelection
						.userVerification as UserVerificationRequirement
				},
				attestation: publicKey.attestation as AttestationConveyancePreference
			}
		});
	} catch (error) {
		if (cancelled(error)) throw new PasskeyCancelled();
		throw error;
	}
	if (!(credential instanceof PublicKeyCredential)) throw new PasskeyCancelled();

	const response = credential.response as AuthenticatorAttestationResponse;
	const publicKeyBytes = response.getPublicKey();
	if (!publicKeyBytes) throw new Error('This authenticator uses an unsupported key type.');
	return {
		challengeId: options.challengeId,
		name,
		credentialId: bytes(credential.rawId),
		clientDataJson: bytes(response.clientDataJSON),
		authenticatorData: bytes(response.getAuthenticatorData()),
		publicKey: bytes(publicKeyBytes),
		publicKeyAlgorithm: response.getPublicKeyAlgorithm(),
		transports: response.getTransports?.() ?? []
	};
}

export async function usePasskey(
	options: PasskeyRequestOptions
): Promise<Init<typeof PasskeyAssertionRequestSchema>> {
	const { publicKey } = options;
	if (!publicKey) throw incomplete();
	let credential: Credential | null;
	try {
		credential = await navigator.credentials.get({
			publicKey: {
				challenge: bytes(publicKey.challenge),
				rpId: publicKey.rpId,
				timeout: publicKey.timeout,
				userVerification: publicKey.userVerification as UserVerificationRequirement,
				allowCredentials: publicKey.allowCredentials.map(descriptor)
			}
		});
	} catch (error) {
		if (cancelled(error)) throw new PasskeyCancelled();
		throw error;
	}
	if (!(credential instanceof PublicKeyCredential)) throw new PasskeyCancelled();

	const response = credential.response as AuthenticatorAssertionResponse;
	return {
		challengeId: options.challengeId,
		credentialId: bytes(credential.rawId),
		clientDataJson: bytes(response.clientDataJSON),
		authenticatorData: bytes(response.authenticatorData),
		signature: bytes(response.signature),
		userHandle: response.userHandle ? bytes(response.userHandle) : undefined
	};
}
