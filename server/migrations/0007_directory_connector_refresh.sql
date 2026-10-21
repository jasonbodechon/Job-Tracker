UPDATE company_directory
SET careers_url = 'https://apply.careers.microsoft.com/careers?domain=microsoft.com',
    verification_status = 'Pending',
    verification_error = NULL,
    next_verification_at = now(),
    updated_at = now()
WHERE id = 'microsoft';

UPDATE company_directory
SET verification_status = 'Pending',
    verification_error = NULL,
    next_verification_at = now(),
    updated_at = now()
WHERE id = 'google';
